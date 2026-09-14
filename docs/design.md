# Design language

How eaten.at looks and why. The direction is **Campari**, adopted on
2026-09-12 from a design handoff (`docs/design-handoff/campari/`): an
aperitivo-hour palette of apricot-blush grounds, oxblood ink, and Campari
red for actions; a display serif over a friendly sans, with a mono for
metadata. Classy but approachable: a food magazine's voice with a modern
app's ergonomics.

The handoff fixes style, not layout. Its tokens are applied exactly; the
page structure is eaten.at's own and predates it. The reference page in the
handoff folder opens in a browser as plain HTML (its viewer runtime,
`support.js`, was not kept and is not needed).

The stylesheet is `crates/eaten-at-web/static/app.css`, a single file
in cascade layers with no build step. Everything named here is a token or
a class in that file.

## Principles

1. **Reading first.** The write-up is the point. The visit card
   and every piece of chrome sit beside the text and stay out of its way.
2. **Three voices.** Titles and names are set in the display serif
   (Instrument Serif). Everything read or pressed, prose and the
   interface alike, is the sans (DM Sans). Everything *about* the
   content (dates, handles, DIDs, URLs, labels) is small, faint mono
   (JetBrains Mono). A reader can tell content from chrome by typeface
   alone.
3. **Accent for actions and ratings.** Campari red is for links,
   buttons, active states, and the rating. Never a large fill, a heading,
   or a border (except a field's error state and the form error's rule).
4. **Cards on blush.** Things a reader picks between (write-ups,
   publications) are cards: a raised surface, a hairline border, a 12px
   radius, and one soft shadow. Sections within a page are separated by
   hairlines and space. Borders do the work; shadows stay faint.
5. **Same shape everywhere.** Every page opens the same way: a small
   mono kicker, then the title, then the content. A listing card is a
   small document page. A chooser card is a small listing card.
6. **A publication recolours, never restructures.** An author's theme
   replaces ground, ink, and accent; layout, type, spacing, and shape are
   fixed.
7. **Server-rendered, no script.** Nothing here needs JavaScript, with
   two exceptions: the editor's place search needs a point and only the
   browser has one, so a small island asks for the location (D35); and
   the sign-in and landing pages suggest handles as you type, from the
   Bluesky AppView, which only the browser can do without a round trip
   (D42). Without either, the search runs near the author's last visit,
   a place can always be entered by hand, and a handle typed in full
   works as it did. Fonts are self-hosted and content-hashed, so no
   page makes a third-party request except those two pages' one call
   to the AppView, which their policy alone allows; the strict CSP
   holds everywhere else.
8. **Light only.** Campari is one palette. The site does not follow the
   system's dark preference.

## Palette

The default palette is Campari's, exact. There are no pure whites, pure
blacks, or grays: every neutral is in the blush and brown family.

| Token | Hex | Use |
|---|---|---|
| `--color-bg` | `#F7E7DC` | page ground (apricot blush) |
| `--color-raised` | `#FCF3EB` | cards, fields, panels, the form error |
| `--color-sunken` | `#F1DCCC` | notices, `pre`, secondary-button hover fill |
| `--color-ink` | `#40201C` | titles, strong text (oxblood) |
| `--color-ink-secondary` | `#5E3C31` | prose, excerpts, comment text |
| `--color-ink-muted` | `#7E574A` | ledes, field labels and hints, secondary buttons, tags |
| `--color-ink-faint` | `#A87F6C` | the mono metadata voice: dates, handles, URLs, kickers |
| `--color-border` | `#E5C9B8` | every hairline and every raised surface's border |
| `--color-border-strong` | `#D9B49C` | a card's border on hover or focus-within |
| `--color-accent` | `#C41E2F` | links, the primary button, active states |
| `--color-accent-fg` | `#FCF3EB` | text on the accent |
| `--color-accent-soft` | `#E8804F` | secondary accent for highlights and badges (unused so far) |
| `--color-accent-hover` | accent, 8% darker | link and primary-button hover |

Contrast on the default grounds:

- The ink shades through `ink-muted` pass 4.5:1 on `bg` and `raised`.
- `ink-faint` does not (about 2.9:1). It is only for small mono
  metadata, never for anything a reader must read to use the page. A
  field hint is read, so it is `ink-muted`.
- The accent passes on `bg` (4.9:1) and `raised`, but not on `sunken`
  (4.44:1). **Sunken panels carry no accent text**, which is why a
  notice's links are ink and underlined.

### Publication themes

A publication's `basicTheme` gives four colors. `theme.rs` clamps them to
WCAG AA and emits six channels on `:root[data-theme]` as bare integers:

| Channel | Source |
|---|---|
| `--theme-bg`, `--theme-fg`, `--theme-accent`, `--theme-accent-fg` | the author's colors, clamped |
| `--theme-raised` | the ground lifted toward white: halfway on a light ground, 6% on a dark one |
| `--theme-sunken` | the ground sunk toward black: 5% on a light ground, 30% on a dark one |

The raised and sunken surfaces are derived in Rust because which way they
move depends on whether the ground is light, which CSS cannot ask.
Everything else is mixed from the channels in the stylesheet: the ink
shades are ink mixed into the ground (88%, 76%, and 58%), the borders 16%
and 28%. A dark author palette therefore gets light shades and borders
automatically.

The clamp checks ink against the ground and both surfaces, and the accent
against the ground and the raised surface (the sunken rule above).

## Type

| Face | Weights | Role |
|---|---|---|
| Instrument Serif | 400, roman and italic | display: page titles, names, card titles, the masthead, legends, empty states |
| DM Sans | 400–700 variable, roman and italic | body: prose, the interface, buttons, labels, tags |
| JetBrains Mono | 400–500 | metadata: dates, handles, DIDs, URLs, kickers, the document footer |

Sizes are fluid between a 390px and a 1080px viewport.

| Token | Mobile → desktop | Use |
|---|---|---|
| `--text-mono` | 10 → 11 | metadata lines, bylines, the site footer |
| `--text-label` | 11 → 12 | kickers, the dateline, the document footer |
| `--text-button` | 12 → 13 | buttons, tags, pagination |
| `--text-small` | 13 → 14 | excerpts, comment text, field labels and hints, notices |
| `--text-body` | 14 → 15 | the interface |
| `--text-lede` | 15 | ledes |
| `--text-prose` | 17 → 18 | a write-up's prose and the editor's body field |
| `--text-card` | 20 → 22 | card titles, the place name, legends, the nameplate's description |
| `--text-masthead` | 20 → 22 | the running head |
| `--text-heading` | 22 → 28 | h2 inside prose |
| `--text-title` | 34 → 44 | the page title (h1) |
| `--text-nameplate` | 44 → 56 | a publication's name on its own front page |

Rules:

- The display serif is only ever weight 400, with tight leading
  (1.0–1.1). Hierarchy comes from size, not weight.
- Body leading is 1.55; prose is 1.6.
- Prose is the one departure from Campari's 14–15px body scale. A
  write-up runs long, so it is set at 17→18px in `ink-secondary`, at
  `--measure` (66ch).
- The mono voice is `ink-faint`, with `letter-spacing: 0.02em`. A
  **kicker** (`.kicker`) is the same voice in capitals at 0.08em,
  sitting above a title. It is the only place capitals appear.
- Italic display serif marks a *description* (a publication's
  description on its nameplate) or an *absence* (an empty state).

## Space and shape

An 8px grid: `--space-xs` 4, `-s` 8, `-sm` 12, `-m` 16, `-l` 24, `-xl` 40.
Section spacing is 40px (`--section-gap`), card padding 16→20px
(`--card-pad`), and inline gaps 8–12px.

| Token | Mobile → desktop | Use |
|---|---|---|
| `--page-top` / `--page-bottom` | 32/40 → 48/56 | main's vertical padding |
| `--page-inline` | 20 → 24 | side padding |
| `--title-gap` | 24 → 32 | below a page title, before content |
| `--footer-gap` / `--footer-pad` | 32/16 → 40/24 | above and inside the document footer |
| `--card-gap` | 12 → 16 | between the parts of a card |

| Radius | Value | Use |
|---|---|---|
| `--radius` | 12px | cards, fields, panels |
| `--radius-nested` | 10px | `pre` inside a card |
| `--radius-s` | 6px | small chips and swatches |
| `--radius-pill` | 999px | buttons, tags, pagination, the skip link |

Elevation: every raised surface has a 1px `--color-border`. Cards alone add
`--shadow-card` (`0 2px 8px` oxblood at 6%).

Motion: 180ms ease-out (`--ease`) on color, background, and border only.
Nothing lifts, scales, or slides, and `prefers-reduced-motion` removes even
that.

## Components

**Masthead** (`.site-header`, `.site-name`). One centred name in the
display serif, a hairline beneath. On the landing page, chooser, and
status pages it is the site's name and leads home. On a publication's
front page it is still the site's name, because the front page carries
its own nameplate. On the publication's inner pages (document, tag) it is
the publication's name and leads to the front page: a running head.

**Nameplate** (`.nameplate`). A publication's front page opens with its
name at `--text-nameplate`, centred; a mono **dateline** (`.dateline`) of
author, site, and rss separated by middle dots; the description as an
italic display-serif lede; the tag chips. A hairline closes it.

**Page head** (`.page-head`). Every other page opens with an optional
kicker, the h1, and an optional lede (`.lede`, `ink-muted` sans at 15px)
or note (`.meta`). The document page's kicker is its date; the tag page's
is "Tag"; a status page's is "Error 404"; the chooser's is
"Publications". The one exception is the editor for a new write-up,
which has no heading until a place is chosen; editing an existing one
opens with an "Edit" kicker and the write-up's title.

**Card** (`.listing-item article`, `.chooser-item`, `.visit-card`).
Raised surface, 1px border, 12px radius, card padding, the card shadow.
On hover or focus-within the border darkens to `border-strong`. Nothing
lifts or moves.

**Listing card** (`.listing-item`). Cards in a column, 16px apart. The
first photo as a small square thumbnail at the left (`.listing-thumb`,
`--thumb-small`, 10px radius) when there is one; right, a kicker date,
the title in the display
serif at card size, the place name as a mono meta line with the rating
marks beside it (`.listing-place`; the name is left out when it is the
title, which it is by default), and the excerpt in the sans at small
size in `ink-secondary`. It is the document page in miniature and in the
same order.

**Chooser card** (`.chooser-item`). Name in the display serif at card
size, description in `ink-secondary`, the publication's URL in the mono
voice. Settings reuses the same card, one of them, with its form inside.

**Place result** (`.result-item`). A card in the editor's choosing
state: the place's name in the display serif at card size, then the
address, distance, and category in the mono voice, and a secondary
"Write about this place" pill at the right. Things a reader picks
between are cards.

**Photo grid** (`section.photos`, `.photo-grid`). Between the visit card
and the prose: the visit's photos as square 400px thumbnails in an
auto-filling grid (8.5rem columns, 8px gaps, 10px radius, a hairline),
each a link to its full rendition. Nothing for a visit without photos.

**Visit card** (`.visit-card`). A card between the page title and the
prose: the place name (`.place-name`) in the display serif. Under the name, in the mono voice, the visit date, the
meal, and the price band as dollar signs, separated by middle dots
(`.visit-meta`); then the address in the sans at small size
(`.place-address`); then the rating.

**Rating** (`.rating`). The verdict as plus signs, one per step, with the
word beside it: `+` Solid, `++` Recommended, `+++` Strongly Recommended,
`++++` Can't Miss. The sans at 700 and button size in the accent on the
card; at the mono size in a listing card, where only the marks show and
the word is the accessible label. The marks are hidden from assistive
technology; the word is what is read. An unrated visit shows nothing. The
same glyphs label the editor's rating radios.

**Prose** (`.prose`). The write-up, at measure and at prose size. Headings
are the display serif in `ink`. Links keep a faint underline at rest, in
the accent at 35%, because color alone does not mark a link inside a
paragraph. Elsewhere links are not underlined until hovered.

**Comments** (`section.comments`). Between the prose and the footer,
hairline above: a "Comments on Bluesky" kicker, then the replies as
hairlined rows. Each row is a byline over the text:

- the author's name in the sans at 500 weight, in `ink`
- the `@handle` and the date (linked to the reply) in faint mono
- the text in the sans at small size, in `ink-secondary`

Replies to replies are indented up to three levels. "No comments yet."
is the empty state, and "Reply on Bluesky →" is in the accent.

**Document footer** (`.doc-footer`). Mono, hairline above. Row one: the
"Links" label (faint) and links (accent), with the comments link pushed
right with an arrow. Row two: tag chips, then edit, rss, and the author
pushed right as muted links.

**Tag chip** (`.tag`). An outlined pill: 1px border, the sans at 500
weight and button size, `ink-muted`. On hover it fills with `sunken` and
the text turns `ink`. The same chip on the front page and in the document
footer.

**Buttons.**
- **Primary** (`button`, `.button`): a pill filled with the accent,
  `accent-fg` text, the sans at 700. It darkens on hover. At most one per
  page.
- **Secondary** (`.button-secondary`, `.button-link`, and a
  `.link-button` placed directly in an `.actions` row): a transparent
  pill with a 1px border, `ink-muted` text, the sans at 500. It fills with
  `sunken` on hover. Any second action is a secondary pill, with a leading
  arrow when it goes back.
- **Touch targets:** below 40rem every pill is at least 44px tall.

Actions sit in an `.actions` row with 8–12px gaps.

**Link button** (`.link-button`). A button dressed as an inline link, for
a POST that sits in a sentence (sign out, restore a draft, remove a row).

**Field** (`input`, `textarea`, `select`). A raised well, 1px border, 12px
radius, placeholder in `ink-faint`. The border darkens on hover and takes
the accent when invalid. The text is never smaller than 16px, so mobile
browsers do not zoom on focus. Checkboxes and radios take the accent.

**Live find** (`.find`, `.find-results`). On the author's home, the
results under the find form are swapped in after a pause in typing,
without a reload; the address bar follows, and the form still submits
as a form. The results region is `aria-live`.

**Combobox** (`.combobox-list`, `.combobox-option`). A listbox an island
puts under a text field: a raised panel with the card's border, radius,
and shadow, one option per row with the name in the sans and a detail
(the handle) in the mono voice. The active option fills with `sunken`.
It appears only with JavaScript on and only while there are matches;
the field it sits under works without it.

**Notice** (`.notice`). A sunken panel, 12px radius, the sans at small
size in `ink-muted`. Its links and link buttons are `ink` and underlined,
never the accent.

**Form error** (`.form-error`). A raised panel with a border, a 3px accent
rule on the leading edge, and `ink` text.

**Empty state** (`.empty`). One italic display-serif line in `ink-muted`,
centred, with room around it, where the content would have been.

**Pagination** (`.pagination`). Two secondary pills: "← Newest" left and
"Older →" right.

**Site footer** (`.site-footer`). Faint mono, centred, hairline above:
one sentence saying what the site is.

## Page recipes

| Page | Masthead | Opens with | Then |
|---|---|---|---|
| Landing `/`, signed out | site | page head: h1 pitch, lede | one primary "Sign in" with a small aside on its baseline; a hairline; the lookup form with a secondary "Read" pill, its hint, and handle suggestions as you type; a note in the metadata voice |
| Landing `/`, signed in | site | page head: the handle in the mono voice where a kicker goes, h1 "Where did you eat?" | one primary "Write a new visit"; "Your publication": a small nameplate (name linked to the front page, address and rss in the mono voice), the find form with a secondary "Find" pill and the tag chips under it, a "Recent write-ups" (or "Matching “q”" with a secondary "Clear") kicker over the listing cards, "All write-ups →" when there are more; or, with no publication yet, one lede saying what it will be; then a hairline and one quiet line, Settings · Sign out |
| Publication front page | site | nameplate | listing, notice if truncated, pagination |
| Tag page | publication | page head: "Tag" kicker, h1 "Tagged “x”", scope note | listing, pagination |
| Document | publication | kicker date, h1 title | visit card, prose, comments (when the document names a Bluesky post), footer |
| Chooser (`/at/{did}/`) | site | page head: "Publications" kicker, h1 author, lede | chooser cards |
| No publications | site | page head: "Publications" kicker, h1 author | empty state |
| Interstitial | site | page head: "Content warning" kicker, h1 | the labels, note, actions |
| Lookup error | site | page head: "Lookup" kicker, h1 | the form with its error |
| Sign in | site | page head: "Sign in" kicker, h1, lede | handle form (the lookup form's shape) with "Start typing your handle…" and a served hint under it; suggestions from Bluesky as you type; errors in place |
| Signing in | site | page head: "Signing in" kicker, h1 "Continuing to host" | one primary button; the page refreshes itself onward |
| Sign-in failed | site | page head: "Sign in" kicker, h1 | one line, secondary "← Try again" |
| Status page | site | page head: "Error nnn" kicker, h1 | detail, secondary "← Back to the start" |
| Editor `/write`, choosing | site | page head: "Write" kicker, h1 "Where did you eat?" | the search box with one primary "Search", a status line for where the search looks, the results as place cards, "Not listed? Enter it by hand", and the Overture attribution |
| Editor `/write`, writing | site | page head: "Write" kicker and the place's name (new), or "Edit" kicker and the write-up's title; a form-error summary when needed | optional preview (the document as readers see it, on a raised panel under a "Preview" kicker); then the form: write-up pane left, visit pane right (Place, Visit, Links, Details, Bluesky groups), stacked under 56rem |
| Photos `/write/{rkey}/photos` | site | page head: "Photos" kicker, h1 "Photos of {place}", lede ("Published. Add photos now, or skip" after a first publish) | the photos as rows on raised cards (thumbnail, alt text field, "Move up", "Move down", "Remove" link buttons), or "No photos yet."; the file input with its hint about re-encoding; one primary "Add photos", secondary "Save alt text", and "Skip for now" / "Done" / "← Back to the write-up" |
| Delete `/write/{rkey}/delete` | site | page head: "Delete" kicker, h1 "Delete “title”?", lede saying what happens | a ticked choice "Also delete the Bluesky post" when there is one to delete (a note when this sign-in may not), one primary button, secondary "← Keep it" |
| Crosspost `/write/{rkey}/crosspost` | site | page head: "Bluesky" kicker, h1 "Post “title” to Bluesky" (or "… is on Bluesky"), lede | the post text field and one primary "Post to Bluesky"; or, before permission, one primary "Allow posting and continue"; secondary "← Skip for now" either way; posted: the thread link and a secondary way back |
| Settings `/settings` | site | page head: "Settings" kicker, h1 "Your publication", lede | one chooser card: the name (linked to the front page) and the current address in the metadata voice, or, before there is one, the name it would get and "Made when you save"; then the form: name, description, the two radio choices for where it lives, one primary "Save" ("Create it" the first time) |

### The editor

The editor is the one page on the wide column (`--column-wide`): two
panes need the room.

- **Controls** are plain form elements. A field's label is the sans at
  500 weight in `ink-muted`; a group's legend is the display serif at
  card size.
- **Hints** are the sans at small size in `ink-muted`.
- **Problems** are a `.field-error` line in the accent directly under the
  control, and the control's border takes the accent too.
- **Choosing a place** comes first for a new write-up. The Place group
  of the writing state opens with a mono line saying where the place
  came from ("Matched to an Overture Maps listing." or "Entered by
  hand") and a "Change place" link button that returns to choosing with
  everything else kept.
- **Repeated fields** (links) are rows separated by hairlines, each
  ending in a "Remove" link button. Adding a row is a "+ Add a link"
  link button.
- **Choices.** The date is a plain date input. The price band, the meal,
  and each link's kind ("Official site" or "Other") are selects. A
  select never offers free text; where a record carries a value from
  another client's vocabulary, that value appears as its own selected
  option so it survives the edit (D31). The rating is a radio group of
  "Unrated" and the four steps, each labelled with its marks and its
  word.
- **Actions:** one primary button, "Publish" (or "Save changes").
  "Preview", "Photos", and "Delete" are secondary pills. A document's
  own author sees "edit" and "photos" links among the footer's muted
  links.
- **Bluesky group:** the visit pane ends with a `.choice` box "Also
  post to Bluesky", the post text with the place name as its
  placeholder, and a hint. Once posted, the group is one line linking the
  thread.
- **Drafts:** with JavaScript on, a draft kept on the device is offered
  back in a `.notice.restore` banner at the top of the form. It is one
  line and two link buttons, "Restore it" and "Discard it".

Signed in, the landing page is the author's home (plan 11). Its last
line, under a hairline, is the only place settings and sign-out appear:
"Settings · Sign out" in the metadata voice, the second a link button.
Signed out there is no account line: the page's primary action is
"Sign in" (plan 09).

## Accessibility and constraints

- Landmarks on every page: skip link, `header`, `main#main`, `footer`;
  `nav` elements carry an `aria-label` naming their scope ("Tags in this
  publication", "Links", "Pagination").
- Focus is a 2px accent outline, offset 2px, on every focusable thing.
- Contrast: see the palette notes. Publication themes are clamped to
  WCAG AA (`theme::MIN_CONTRAST`) on every surface their text sits on.
- `prefers-reduced-motion` disables the transitions; there are no
  animations.
- Fonts are subset to Latin and Latin Extended, use `font-display: swap`,
  and fall back to Georgia, the system sans, and the system mono. The
  licences are in `static/fonts/OFL.txt`.
- No page makes a third-party request and there are no iframes, except
  that the sign-in and landing pages call the Bluesky AppView for handle
  suggestions and their policy alone allows that origin (D42). Campari's handoff names Google Fonts; the
  same files are served from `/static/` instead.
- Nothing scrolls horizontally at a 390px viewport.

## Do and don't

- Do put a date, a handle, a DID, or a URL in the mono voice. Don't set it
  in the sans at body size.
- Do make a thing a reader chooses between a card. Don't box a section of
  a page.
- Do use the accent for a link, the primary button, the rating, or an
  error. Don't use it for a heading, a surface, or on a sunken panel.
- Do use size for hierarchy in the display serif. Don't bold it.
- Do let a page be short. Don't fill an empty state with instructions.
- Do keep the column at 720px. Don't add a wide variant for a listing.
