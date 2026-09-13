# Design language

How eaten.at looks and why. The direction is inherited from
album-report's 2026-09-07 mocks ("paper journal"): a warm cream ground, a
serif for reading, a monospace for metadata, one rust accent, and a centred
masthead. The document page was built to the mock first; this document
writes the rules down so every other page follows the same ones.

The stylesheet is `crates/eaten-at-web/static/app.css`, a single file
in cascade layers with no build step. Everything named here is a token or
a class in that file.

## Principles

1. **Reading first.** The write-up is the point. The subject card
   and every piece of chrome sit beside the text and stay out of its way.
2. **Two voices.** Prose and titles are set in the serif (Newsreader).
   Everything that is *about* the content, dates, bylines, links out,
   labels, notes, is set small in the mono (Spline Sans Mono). A reader can
   tell content from chrome by typeface alone.
3. **One accent.** Rust is for links and the one filled button a page may
   have. Nothing else is coloured; emphasis comes from size, weight, and
   space.
4. **Paper, not panels.** Sections are separated by hairlines and
   whitespace, never by boxes or cards. The only filled shapes are the tag
   chip, the notice, and the button.
5. **Same shape everywhere.** Every page opens the same way: a small
   mono line, then the title, then the content. A listing row is a small
   document page. A chooser row is a small listing row.
6. **A publication recolours, never restructures.** An author's theme
   replaces the four palette channels; layout, type, and spacing are
   fixed.
7. **Server-rendered, no script.** Nothing here needs JavaScript. Fonts
   are self-hosted and content-hashed, so no page makes a third-party
   request and the strict CSP holds.

## Palette

Colours are stored as RGB channels (`--theme-*`) so alpha can be applied,
and every visible colour derives from four of them. A publication's
`basicTheme` overrides the four; the derived colours follow.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--theme-bg` | 250 247 241 (cream) | 22 19 16 | page ground |
| `--theme-fg` | 33 28 20 (warm black) | 242 237 227 | text |
| `--theme-accent` | 156 63 26 (rust) | 224 123 74 | links, the button |
| `--theme-accent-fg` | white | 28 16 5 | text on the accent |

Derived, all from `--theme-fg` at an alpha so they work on any ground:

| Token | Alpha | Use |
|---|---|---|
| `--color-muted` | 0.70 | metadata voice, ledes, bylines, excerpts' quieter siblings |
| `--color-quiet` | 0.52 | labels within metadata ("Links"), separators, placeholders |
| `--color-line` | 0.10 | hairlines between sections and rows |
| `--color-frame` | 0.13 | borders that enclose something (the blockquote rule) |
| `--color-surface` | 0.06 | the fill behind a tag chip, a notice, a code block |
| `--shadow-cover` | 0.18 | the one shadow, under cover images |

Dark mode inverts the default palette with the same warmth. A publication
theme is used as given in both modes, contrast-clamped by `theme.rs`.

## Type

| Face | Role |
|---|---|
| Newsreader (400–700, italic 400–600) | reading: prose, titles, subject names, ledes |
| Spline Sans Mono (400–600) | metadata: dates, bylines, labels, links out, notes, buttons, form labels |

Sizes are fluid between a 390 px and a 1080 px viewport; the endpoints
are the mobile and desktop mocks.

| Token | Mobile → desktop | Use |
|---|---|---|
| `--text-mono` | 11 → 12 | the whole metadata voice |
| `--text-small` | 14 → 17 | bylines, listing excerpts |
| `--text-body` | 17 → 19 | prose, ledes |
| `--text-masthead` | 16 → 19 | the running head |
| `--text-lg` | 19 → 26 | the subject title on the card; listing and chooser titles |
| `--text-title` | 27 → 40 | the page title (h1) |
| `--text-nameplate` | 32 → 48 | a publication's name on its own front page |
| `--text-heading` | 22 → 30 | h2 inside prose |

Rules:

- Titles are 600 weight, tight leading (1.05–1.15), a hair of negative
  tracking, and `text-wrap: pretty` or `balance`.
- The metadata voice always carries `letter-spacing: 0.04em`. A
  **kicker** (`.kicker`) is the metadata voice in capitals above a title;
  it is the only place capitals appear.
- Italic serif marks a *credit or description* (a listing's subject line,
  a publication's description, an empty state), never emphasis in chrome.
- Prose runs at `--measure` (66ch) and 1.65 leading. The column is 720 px
  (`--column`); nothing on a page is wider than that.

## Space

Spacing tokens are fluid like the type. The ones that shape a page:

| Token | Mobile → desktop | Use |
|---|---|---|
| `--page-top` / `--page-bottom` | 26/34 → 48/56 | main's vertical padding |
| `--page-inline` | 20 → 24 | side padding |
| `--title-gap` | 22 → 34 | below a page title, before content |
| `--section-gap` | 28 → 44 | between major sections (nameplate → listing, listing → pagination) |
| `--footer-gap` / `--footer-pad` | 26/16 → 44/20 | above and inside the document footer |
| `--card-gap` | 14 → 20 | cover to text |
| `--cover-size` / `--cover-small` | 84/72 → 120/96 | cover on the card / in a listing row |

Within a block the fixed steps `--space-xs` (4) … `--space-xl` (48) apply.

## Components

**Masthead** (`.site-header`, `.site-name`). One centred name in the
serif, a hairline beneath. On the landing page, chooser, and status
pages it is the site's name and leads home. On a publication's front
page it is still the site's name, because the front page carries its
own nameplate. On the publication's inner pages (document, tag) it is
the publication's name and leads to the front page: a running head.

**Nameplate** (`.nameplate`). A publication's front page opens with its
name at `--text-nameplate`, centred; a mono **dateline** (`.dateline`)
of author, site, and rss separated by middle dots; the description as
an italic lede; the tag chips. A hairline closes it.

**Page head** (`.page-head`). Every other page opens with an optional
kicker, the h1, and an optional lede (`.lede`, muted serif at body
size) or note (`.meta`). The document page's kicker is its date; the
tag page's is "Tag"; a status page's is "Error 404"; the chooser's is
"Publications".

**Subject card** (`.subject-card`). Cover (with the one shadow) beside the
subject title, no box, sitting between the page title and the prose.

**Prose** (`.prose`). The write-up, at measure.

**Comments** (`.comments`). Between the prose and the footer, hairline
above: a "Comments on Bluesky" kicker, then the replies as hairlined
rows, each a mono byline (name in the foreground weight, `@handle`
quiet, the date linked to the reply) over the text in the serif at the
small size, replies to replies indented up to three levels; "No
comments yet." as the empty state; "Reply on Bluesky →" in the accent.

**Document footer** (`.doc-footer`). Mono, hairline above. Row one:
"Links" label (quiet) and links (accent), the comments link
pushed right with an arrow. Row two: tag chips, then rss and the author
pushed right as quiet links.

**Listing row** (`.listing-item`). Small cover left; right, a kicker
date, the title at `--text-lg`, the subject title in italic small,
and the excerpt in small. Hairlines between rows. It is the document
page in miniature and in the same order.

**Chooser row** (`.chooser-item`). Name at `--text-lg`, description
muted, the publication's URL in the metadata voice. Hairlines between.

**Tag chip** (`.tag`). Mono on `--color-surface`, small radius, muted
text; accent on hover. The same chip on the front page and in the
document footer.

**Button** (`button`, `.button`). Mono, 600, pill, filled with the
accent. At most one per page. Any second action is a **quiet link**
(`.button-link`): mono, muted, with a leading arrow when it goes back.
Actions sit in a `.actions` row.

**Field** (`input`). Serif at body size, one hairline border, a
placeholder in quiet. The lookup form labels the field with a kicker.

**Notice** (`.notice`) and **form error** (`.form-error`). Mono on
`--color-surface`; the error adds a 3 px accent rule on the left and
full-strength text.

**Empty state** (`.empty`). One italic muted line, centred, with room
around it, where the content would have been.

**Pagination** (`.pagination`). Mono, hairline above, "← Newest" left
and "Older →" right.

**Site footer** (`.site-footer`). Mono, quiet, centred, hairline above:
one sentence saying what the site is.

## Page recipes

| Page | Masthead | Opens with | Then |
|---|---|---|---|
| Landing `/` | site | page head: h1 pitch, lede | lookup form, note in metadata voice |
| Publication front page | site | nameplate | listing, notice if truncated, pagination |
| Tag page | publication | page head: "Tag" kicker, h1 "Tagged “x”", scope note | listing, pagination |
| Document | publication | kicker date, h1 title | subject card, prose, comments (when the document names a Bluesky post), footer |
| Chooser (`/at/{did}/`) | site | page head: "Publications" kicker, h1 author, lede | chooser rows |
| No publications | site | page head: "Publications" kicker, h1 author | empty state |
| Interstitial | site | page head: "Content warning" kicker, h1 | the labels, note, actions |
| Lookup error | site | page head: "Lookup" kicker, h1 | the form with its error |
| Sign in | site | page head: "Sign in" kicker, h1, lede | handle form (the lookup form's shape); errors in place |
| Signing in | site | page head: "Signing in" kicker, h1 "Continuing to host" | one filled button; the page refreshes itself onward |
| Sign-in failed | site | page head: "Sign in" kicker, h1 | one line, quiet link back to the form |
| Status page | site | page head: "Error nnn" kicker, h1 | detail, quiet link home |
| Editor `/write` | site | page head: "Write" or "Edit" kicker, h1; a form-error summary when needed | optional preview (the document as readers see it, under a "Preview" kicker); then the form: write-up pane left, subject pane right, stacked under 56rem |
| Delete `/write/{rkey}/delete` | site | page head: "Delete" kicker, h1 "Delete “title”?", lede saying what happens | a ticked choice "Also delete the Bluesky post" when there is one to delete (a note when this sign-in may not), one filled button, quiet "← Keep it" |
| Crosspost `/write/{rkey}/crosspost` | site | page head: "Bluesky" kicker, h1 "Post “title” to Bluesky" (or "… is on Bluesky"), lede | the post text field and one filled "Post to Bluesky"; or, before permission, one filled "Allow posting and continue"; quiet "← Skip for now" either way; posted: the thread link and a quiet way back |
| Settings `/settings` | site | page head: "Settings" kicker, h1 "Your publications", lede | chooser rows: name, current address in the metadata voice, a small form of two radio choices and one filled "Save" |

The editor is the one page on the wide column (`--column-wide`): two
panes need the room. Its controls are plain form elements. Labels are
kickers; hints and legends are the metadata voice; a problem is a
`.field-error` line in the accent directly under its control, and the
control's border takes the accent too. Rows of repeated fields (links)
are separated by hairlines and end in a quiet "Remove"; adding a
row is a quiet "+ Add" link-button. One filled button, "Publish" (or
"Save changes"); "Preview" and "Delete" are quiet. A document's own
author sees a quiet "edit" among the footer's quiet links. The subject
pane ends with a "Bluesky" group: a `.choice` box "Also post to Bluesky",
the post text with the subject title as its placeholder, and a hint in the
metadata voice; once posted, the group is one line linking the thread. With JavaScript on, a
draft kept on the device is offered back in a `.notice.restore` banner
at the top of the form: one line in the metadata voice and two quiet
buttons, "Restore it" and "Discard it".

The landing page also carries an **account line** under the lookup form, in
the metadata voice: "Signed in as @handle" with a sign-out control, or a
sign-in link. A control that must be a `POST` is a `.link-button`: a button
dressed as a link, so the voice stays the same.

## Accessibility and constraints

- Landmarks on every page: skip link, `header`, `main#main`, `footer`;
  `nav` elements carry an `aria-label` naming their scope ("Tags in this
  publication", "Links", "Pagination").
- Focus is a 3 px accent ring, offset 2 px, on every focusable thing.
- Contrast: body text is `--theme-fg` on `--theme-bg`; publication
  themes are clamped to WCAG AA (`theme::MIN_CONTRAST`). The muted
  voice at 0.7 alpha of a passing foreground still passes on the
  default grounds.
- `prefers-reduced-motion` disables the few transitions; there are no
  animations.
- Fonts subset to Latin and Latin Extended, `font-display: swap`, with
  Georgia and the system mono as fallbacks.
- No third-party requests from any page, and no iframes; the CSP allows
  no external origin.

## Do and don't

- Do put a date, a label, or a URL in the metadata voice. Don't set it
  in the serif at body size.
- Do separate with a hairline. Don't draw a box.
- Do use the accent for a link or the button. Don't use it for a
  heading, a border, or a background.
- Do let a page be short. Don't fill an empty state with instructions.
- Do keep the column at 720 px. Don't add a wide variant for a listing.
