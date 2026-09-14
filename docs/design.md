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
page structure is eaten.at's own and predates it. The reference page in the
handoff folder opens in a browser as plain HTML (its viewer runtime,
`support.js`, was not kept and is not needed; the page's Google Fonts
links are the only thing it fetches).

The stylesheet is `crates/eaten-at-web/static/app.css`, a single file
in cascade layers with no build step. Everything named here is a token or
a class in that file.

## Principles

1. **Reading first.** The write-up is the point. The visit's facts and
   every piece of chrome sit beside the text and stay out of its way.
2. **Two voices, and a signature.** Everything read is the serif
   (Newsreader): titles and names at weight 500, text at 400. Everything
   *about* the content (dates, handles, DIDs, URLs, labels, ratings) and
   everything *pressed* (buttons, chips) is small mono (JetBrains Mono),
   in capitals when it labels. The logotype alone is Evantic, always
   lowercase, and it is never used for anything else. A reader can tell
   content from chrome by typeface alone.
3. **Vermilion is scarce.** The accent is for the rating, links, active
   states, and one primary action per page. Never a large fill, a
   heading, a border, or a background (except a field's error state and
   the form error's rule).
4. **Rules, not boxes.** Things a reader picks between (write-ups,
   publications, places, photos) are rows on the paper: a 1px ink rule
   above the first, hairlines between the rest. Sections open with a 1px
   ink rule; mastheads close with a 3px double one. The one raised
   surface is `paper-bright`, for fields, sheets, and the visit's fact
   box, with a hairline and square corners. Nothing has a radius, a
   shadow, a lift, or a scale.
5. **Same shape everywhere.** Every page opens the same way: a small
   mono kicker, then the title, then the content. A listing row is a
   small document page. A chooser row is a small listing row.
6. **A publication recolours, never restructures.** An author's theme
   replaces paper, ink, and vermilion; layout, type, spacing, and shape
   are fixed.
7. **Server-rendered, no script.** Nothing here needs JavaScript. A
   few islands make pages quicker: the sign-in and landing pages
   suggest handles as you type from the Bluesky AppView (D42), the
   editor suggests places as you type through this site's own endpoint
   (D45), the author's home applies its find as you type, and the
   editor keeps a draft. Without any of them, a handle typed in full, a
   Search button, a Find button, and a plain form do the same work.
   Nothing asks the browser for its location: the search looks near
   where the request is from (D44). Fonts are self-hosted and
   content-hashed, so no page makes a third-party request except the
   two handle pages' one call to the AppView, which their policy alone
   allows; the strict CSP holds everywhere else.
8. **Light only.** Masthead is one palette. The site does not follow the
   system's dark preference.

## Palette

The default palette is Masthead's, exact. There are no pure whites, pure
blacks, or grays: every neutral is in the ivory and umber family, and
there is no hue but vermilion.

| Token | Hex | Use |
|---|---|---|
| `--color-paper` | `#F6F1E5` | page ground (ivory stock) |
| `--color-paper-bright` | `#FDFBF4` | fields, sheets, panels, the visit's fact box, the text on ink and vermilion fills |
| `--color-ink` | `#1C1914` | titles, strong text, rules, the logotype, secondary buttons |
| `--color-ink-body` | `#332E24` | a write-up's text, excerpts, comment text |
| `--color-ink-soft` | `#4A4336` | ledes, hints, field labels, notices, quiet links |
| `--color-stone` | `#988D75` | the mono metadata voice: dates, handles, URLs, kickers, placeholders |
| `--color-hairline` | `#DDD5C2` | minor rules, every bright surface's border |
| `--color-vermilion` | `#D8401F` | links, the primary button, active states, the rating |

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
  of it is either short and tracked capitals (the rating, a button), a
  link that underlines on hover, or a link inside prose that keeps a
  faint underline at rest. Darkening it to reach 4.5 is a one-token
  change if that is ever wanted; `theme.rs` has a test recording the
  ratio so the number does not drift unnoticed.

### Publication themes

A publication's `basicTheme` gives four colors. `theme.rs` clamps them to
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
| Newsreader | 200–800 variable with optical sizes, roman and italic; 500 and 400 used | titles, names, and the running head at 500; text, ledes, hints, notices, comment text, and fields at 400 |
| JetBrains Mono | 400–500 | metadata, kickers, labels, buttons, chips, the rating, the document footer |

Newsreader's optical size axis follows the font size, so small text gets
the text cut and titles the display cut without anything asking for it.
Sizes are fluid between a 390px and a 1080px viewport.

| Token | Mobile → desktop | Use |
|---|---|---|
| `--text-mono` | 10 → 11 | metadata lines, bylines, the site footer, tag chips |
| `--text-label` | 10 → 11 | kickers, field labels, the tagline: tracked capitals |
| `--text-button` | 11 → 12 | buttons, pagination, the rating, the document footer |
| `--text-small` | 14 | excerpts, comment text, hints, notices, the address |
| `--text-body` | 14 → 15 | the interface, choice labels, a comment's author |
| `--text-lede` | 16 | ledes, a publication's description |
| `--text-prose` | 16 | a write-up's text and the editor's body field |
| `--text-card` | 20 → 22 | row titles, the place name, legends, the author's own nameplate, empty states |
| `--text-heading` | 22 → 24 | h2 inside prose |
| `--text-masthead` | 22 → 26 | the running head |
| `--text-title` | 28 → 34 | the page title (h1) |
| `--text-nameplate` | 32 → 40 | a publication's name on its own front page |
| `--text-logotype` | 40 → 52 | the logotype |

Rules:

- Titles are the serif at 500 with tight leading (1.1–1.15). Hierarchy
  comes from size; nothing is bolder than 500 except `strong` in prose
  (600).
- Text leading is 1.55; prose, excerpts, and comment text 1.6.
- Prose is 16px, the top of the handoff's 14–16 range, in `ink-body`, at
  `--measure` (640px).
- The mono voice is `stone` with `letter-spacing: 0.04em`. Capitals are
  always tracked: kickers and field labels at 0.16em, buttons at 0.1em,
  the rating at 0.14em, the tagline at 0.22em. A **kicker** (`.kicker`)
  sits above a title or names a section; it is one of the few places
  capitals appear, with buttons, labels, the rating, and the tagline.
- Italic serif marks a *description* (a publication's description on its
  nameplate) or an *absence* (an empty state).

## Space and shape

An 8px grid: `--space-xs` 4, `-s` 8, `-sm` 12, `-m` 16, `-l` 24, `-xl` 40.
Section spacing is 40→56px (`--section-gap`), sheet padding 16→24px
(`--card-pad`), a row's padding above and below 16→20px (`--row-pad`),
inline gaps 8–12px, and text actions 18px apart (`--action-gap`).

| Token | Mobile → desktop | Use |
|---|---|---|
| `--page-top` / `--page-bottom` | 32/40 → 48/56 | main's vertical padding |
| `--page-inline` | 20 → 32 | side padding |
| `--masthead-top` / `--masthead-bottom` | 24/16 → 36/20 | above and below the masthead's contents |
| `--title-gap` | 24 → 32 | below a page title, before content |
| `--footer-gap` / `--footer-pad` | 32/16 → 40/24 | above and inside the document footer |
| `--card-gap` | 12 → 16 | between the parts of a row or sheet |

Shape: **square corners everywhere.** Three rules do the separating:

| Rule | Value | Use |
|---|---|---|
| `--rule-ink` | 1px solid ink | above a list of rows, above a section, a blockquote's edge |
| `--rule-hairline` | 1px solid hairline | between rows, under the masthead's footer, around every bright surface |
| `--rule-double` | 3px double ink | under the masthead and under a publication's nameplate |

Elevation: none. A bright surface has a hairline and nothing else.

Motion: 160ms ease-out (`--ease`) on color, background, and border only.
Nothing lifts, scales, or slides, and `prefers-reduced-motion` removes even
that.

## Components

**Masthead** (`.site-header`). Centred, closed by the double rule. On the
landing page, chooser, editor, and status pages it is the **logotype**
(`.site-name.logotype`, Evantic at logotype size, leading home) over the
**tagline** (`.tagline`, "The federated table" in tracked mono capitals in
`stone`). On a publication's front page it is still the logotype, because
the front page carries its own nameplate. On the publication's inner pages
(document, tag) it is the publication's name as a **running head**
(`.site-name.running-head`, the serif at 500 and masthead size) leading to
the front page, with no tagline: the logotype is Evantic's only job, and a
publication's name is not the logotype.

**Nameplate** (`.nameplate`). A publication's front page opens with its
own masthead: the name at `--text-nameplate`, centred; a mono
**dateline** (`.dateline`) of author, site, and rss separated by middle
dots; the description as an italic lede; the tag chips. The double rule
closes it.

**Page head** (`.page-head`). Every other page opens with an optional
kicker, the h1, and an optional lede (`.lede`, `ink-soft` serif at 16px)
or note (`.meta`). The document page's kicker is its date; the tag page's
is "Tag"; a status page's is "Error 404"; the chooser's is
"Publications". The one exception is the editor for a new write-up,
which has no heading until a place is chosen; editing an existing one
opens with an "Edit" kicker and the write-up's title.

**Rows** (`.listing`, `.chooser`, `.results`, `.photo-manage`). A column
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
description in `ink-body`, the publication's URL in the mono voice.
Settings reuses the same row, one of them, with its form inside.

**Place result** (`.result-item`). A row in the editor's choosing state:
the place's name in the serif at row-title size, then the address,
distance, and category in the mono voice, and a secondary "Write about
this place" button at the right.

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

**Prose** (`.prose`). The write-up, at measure and at prose size, in
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

**Buttons.** Rectangles in the mono voice at button size, capitals
tracked at 0.1em, padded 11px by 22px. Hovering inverts fill and outline.
- **Primary** (`button`, `.button`): filled with vermilion, `paper-bright`
  text. At most one per page: it is the page's one action. On hover the
  fill drops and the outline and text are vermilion.
- **Secondary** (`.button-secondary`, `.button-link`, `.pagination a`,
  and a `.link-button` placed directly in an `.actions` row): a 1px ink
  outline, `ink` text, transparent. On hover it fills with ink and the
  text is `paper-bright`. Any second action is a secondary, with a
  leading arrow when it goes back.
- **Touch targets:** below 40rem every button is at least 44px tall.

Actions sit in an `.actions` row with 8–12px gaps.

**Link button** (`.link-button`). A button dressed as an inline link, for
a POST that sits in a sentence (sign out, restore a draft, remove a row).

**Field** (`input`, `textarea`, `select`). A bright well with a hairline,
square, the serif at 16px or more (so mobile browsers do not zoom on
focus), placeholder in `stone`. The border turns `stone` on hover and
vermilion when invalid. Checkboxes and radios take the accent. A field's
**label** is a UI label: tracked mono capitals in `ink-soft`.

**Live find** (`.find`, `.find-results`). On the author's home, the
results under the find form are swapped in after a pause in typing,
without a reload; the address bar follows, and the form still submits
as a form. The results region is `aria-live`.

**Combobox** (`.combobox-list`, `.combobox-option`). A listbox an island
puts under a text field: a bright sheet with a hairline, one option per
row with the name in the serif and a detail (the handle) in the mono
voice. The active option takes the paper. It appears only with
JavaScript on and only while there are matches; the field it sits under
works without it.

**Notice** (`.notice`). A bright sheet with a hairline, the serif at
small size in `ink-soft`. Its links are the accent like any other.

**Form error** (`.form-error`). A bright sheet with a hairline and a 3px
vermilion rule on the leading edge, `ink` text.

**Empty state** (`.empty`). One italic serif line in `ink-soft`, centred,
with room around it, where the content would have been.

**Pagination** (`.pagination`). Two secondary buttons: "← Newest" left
and "Older →" right.

**Site footer** (`.site-footer`). The mono voice in `stone`, centred,
hairline above: one sentence saying what the site is.

## Page recipes

| Page | Masthead | Opens with | Then |
|---|---|---|---|
| Landing `/`, signed out | logotype | page head: h1 pitch, lede | one primary "Sign in" with a small aside on its baseline; a hairline; the lookup form with a secondary "Read" button, its hint, and handle suggestions as you type; a note in the metadata voice |
| Landing `/`, signed in | logotype | page head: the handle in the mono voice where a kicker goes, h1 "Where did you eat?" | one primary "Write a new visit"; "Your publication": a small nameplate (name linked to the front page, address and rss in the mono voice), the find form with a secondary "Find" button and the tag chips under it, a "Recent write-ups" (or "Matching “q”" with a secondary "Clear") kicker over the listing rows, "All write-ups →" when there are more; or, with no publication yet, one lede saying what it will be; then a hairline and one quiet line, Settings · Sign out |
| Publication front page | logotype | nameplate | listing, notice if truncated, pagination |
| Tag page | running head | page head: "Tag" kicker, h1 "Tagged “x”", scope note | listing, pagination |
| Document | running head | kicker date, h1 title | the visit, photos, prose, comments (when the document names a Bluesky post), footer |
| Chooser (`/at/{did}/`) | logotype | page head: "Publications" kicker, h1 author, lede | chooser rows |
| No publications | logotype | page head: "Publications" kicker, h1 author | empty state |
| Interstitial | logotype | page head: "Content warning" kicker, h1 | the labels, note, actions |
| Lookup error | logotype | page head: "Lookup" kicker, h1 | the form with its error |
| Sign in | logotype | page head: "Sign in" kicker, h1, lede | handle form (the lookup form's shape) with "Start typing your handle…" and a served hint under it; suggestions from Bluesky as you type; errors in place |
| Signing in | logotype | page head: "Signing in" kicker, h1 "Continuing to host" | one primary button; the page refreshes itself onward |
| Sign-in failed | logotype | page head: "Sign in" kicker, h1 | one line, secondary "← Try again" |
| Status page | logotype | page head: "Error nnn" kicker, h1 | detail, secondary "← Back to the start" |
| Editor `/write`, choosing | logotype | page head: "Write" kicker, h1 "Where did you eat?" | the search box ("Start typing…") with suggestions as you type and one primary "Search" for the plain path, a status line for where the search looks ("Searching near Brooklyn"), the results as place rows; or, with no location, a notice that search is off; then, under a hairline, "Or enter it yourself": Name, Address (optional), and a secondary "Continue with this place"; last, the Overture and DB-IP attribution |
| Editor `/write`, writing | logotype | page head: "Write" kicker and the place's name (new), or "Edit" kicker and the write-up's title; a form-error summary when needed | optional preview (the document as readers see it, on a bright sheet under a "Preview" kicker); then the form: write-up pane left, visit pane right (Place, Visit, Links, Details, Bluesky groups), stacked under 56rem |
| Photos `/write/{rkey}/photos` | logotype | page head: "Photos" kicker, h1 "Photos of {place}", lede ("Published. Add photos now, or skip" after a first publish) | the photos as rows (thumbnail, alt text field, "Move up", "Move down", "Remove" link buttons), or "No photos yet."; the file input with its hint about re-encoding; one primary "Add photos", secondary "Save alt text", and "Skip for now" / "Done" / "← Back to the write-up" |
| Delete `/write/{rkey}/delete` | logotype | page head: "Delete" kicker, h1 "Delete “title”?", lede saying what happens | a ticked choice "Also delete the Bluesky post" when there is one to delete (a note when this sign-in may not), one primary button, secondary "← Keep it" |
| Crosspost `/write/{rkey}/crosspost` | logotype | page head: "Bluesky" kicker, h1 "Post “title” to Bluesky" (or "… is on Bluesky"), lede | the post text field and one primary "Post to Bluesky"; or, before permission, one primary "Allow posting and continue"; secondary "← Skip for now" either way; posted: the thread link and a secondary way back |
| Settings `/settings` | logotype | page head: "Settings" kicker, h1 "Your publication", lede | one chooser row: the name (linked to the front page) and the current address in the metadata voice, or, before there is one, the name it would get and "Made when you save"; then the form: name, description, the two radio choices for where it lives, one primary "Save" ("Create it" the first time) |

### The editor

The editor is the one page on the wide column (`--column-wide`): two
panes need the room.

- **Controls** are plain form elements. A field's label is tracked mono
  capitals in `ink-soft`; a group's legend is the serif at 500 and
  row-title size.
- **Hints** are the serif at small size in `ink-soft`.
- **Problems** are a `.field-error` line in the accent directly under the
  control, and the control's border takes the accent too.
- **Choosing a place** comes first for a new write-up: suggestions as
  you type, the plain search, or a name and address by hand, all on
  one page. The Place group of the writing state opens with a mono
  line saying where the place came from ("Matched to an Overture Maps
  listing." or "Entered by hand") and a "Change place" link button
  that returns to choosing with everything else kept.
- **Repeated fields** (links) are rows separated by hairlines, each
  ending in a "Remove" link button. Adding a row is a "+ Add a link"
  link button.
- **Choices.** The date is a plain date input. The price band, the meal,
  and each link's kind ("Official site" or "Other") are selects. A
  select never offers free text; where a record carries a value from
  another client's vocabulary, that value appears as its own selected
  option so it survives the edit (D31). The rating is a radio group of
  "Unrated" and the four steps, each labelled with its word.
- **Actions:** one primary button, "Publish" (or "Save changes").
  "Preview", "Photos", and "Delete" are secondaries. A document's
  own author sees "edit" and "photos" links among the footer's quiet
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
- Focus is a 2px vermilion outline, offset 2px, on every focusable thing.
- Contrast: see the palette notes, including the vermilion shortfall.
  Publication themes are clamped to WCAG AA (`theme::MIN_CONTRAST`) on
  every surface their text sits on.
- `prefers-reduced-motion` disables the transitions; there are no
  animations.
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
- Do use vermilion for a link, the primary button, the rating, or an
  error. Don't use it for a heading, a fill, a border, or a background.
- Do use size for hierarchy in the serif. Don't go bolder than 500.
- Do track capitals. Don't set capitals in the serif, or untracked.
- Do use Evantic for `eaten.at`, lowercase. Don't use it for anything
  else, and don't set the logotype in any other face.
- Do let a page be short. Don't fill an empty state with instructions.
- Do keep the column at 720px. Don't add a wide variant for a listing.
