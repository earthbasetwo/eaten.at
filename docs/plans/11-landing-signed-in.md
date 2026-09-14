# 11 · Landing page, signed in

Feedback 14: *Signed in, the landing page should prioritize writing a
new visit as the main action.*
Feedback 15: *Signed in, the landing page should offer the ability to
view and query your own publication as secondary actions: show the
recent n reviews in a list.*
Feedback 16: *The settings page should be tertiary priority.*

Medium. After plan 08 (the page shows *the* publication) and plan 09
(the signed-out half). Plan 13 changes the cards this page lists, but
this plan works with today's cards.

## The page after

Signed in, `/` is the author's home. In order:

1. **Page head.** Kicker: the handle in the mono voice (`@alice.bsky.social`),
   which is where the "Signed in as" line goes. `h1` "Where did you
   eat?" No lede.
2. **The one primary action.** An `.actions` row with a primary
   `a.button` "Write a new visit" to `/write`.
3. **Your publication.** A section under a kicker "Your publication",
   with the publication's name in the display serif at card size,
   linked to its front page, and its address in the mono voice (the
   hosted host, or the author's domain), with "rss" beside it, like a
   small nameplate. Then:
   - **Find.** A one-line form, `GET /` with `q`, labelled "Find a
     write-up" in the kicker voice, a text field, and a secondary
     "Find" pill. Directly under it, the publication's tag chips as the
     front page shows them, so the two ways of narrowing the list sit
     together; a tag leads to the existing tag page. See "Querying"
     below.
   - **Recent write-ups.** The `listing` component with the newest
     eight visit documents; under it a secondary pill "All write-ups →"
     to the publication's front page. Fewer than eight: no pill. None:
     the empty state "No write-ups yet." in place of the list.
4. **Tertiary.** A hairline, then one metadata-voice line: "Settings ·
   Sign out", the second a link button (a POST). That is the whole
   presence of settings on the page.

No publication yet (plan 08's "not yet" state): the section reads, in
place of the nameplate, "Your publication is made when you publish your
first write-up. It will be called alice.bsky.social at alice.eaten.at;
change that in settings." with the Find form and the list omitted.

## Querying

"Query your own publication" means two things (settled 2026-09-13): a
substring find over the place name, title, and address, and filtering
by tag as the tag pages already do. The tag chips on this page are that
second half; nothing new is built for it beyond placing the chips next
to the Find form. There is no index (D9), and there will not be one for
this; the publication's documents are scanned page by page as the
listing and tag pages already do (`scan_visits`, capped at
`MAX_LIST_SCAN_PAGES`).

- `GET /?q=katz` renders the same page with the list replaced by the
  matches: visit documents whose place name, title, or address contains
  the query, case-folded and trimmed, in the same newest-first order,
  up to a page. The kicker over the list becomes "Matching “katz”" and
  a secondary "Clear" pill returns to `/`. No matches: "Nothing called
  “katz” among your write-ups." Truncated by the scan cap: the existing
  notice sentence about older documents.
- The match is a substring test on three fields, nothing more. Tags
  are filtered through their chips, not through the text field; a
  rating or date filter is not asked for.
- The form is `GET`, so a search is a URL and the back button works.
  Signed out, `?q=` is ignored.

`read.rs` gains `find_visits(identity, publication, query, cursor)`
beside `tagged_listing`, sharing `scan_visits` with a `keep` closure.
Matching lives in `tags.rs`'s neighbour, a small `search.rs`, with its
own tests (case folding, whitespace, a query shorter than two characters
matches nothing).

## Where "recent eight" comes from

`visit_listing` reads a page of twenty; the page shows the first eight
and drops the rest. One request, already cached for five minutes under
`DocumentList`, and the same cache the publication page fills, so
landing after a publish shows the new document at once (the publish
path evicts the listing).

## Design

- `docs/design.md`, page recipes: Landing `/` signed in: "page head:
  handle kicker, h1 | primary Write a new visit | Your publication:
  small nameplate, Find form with the tag chips under it, recent
  listing, All write-ups pill | hairline, Settings · Sign out". The paragraph about the account line
  under the lookup form is replaced by this.
- `app.css`: `.own-publication` (the section), `.own-nameplate` (name at
  card size, address in the mono voice on one wrapping line),
  `.find` (the one-line form, the lookup row's shape), `.tertiary` (the
  closing metadata line). Nothing new in shape or voice: every piece is
  an existing component.

## Touchpoints

- `routes/landing.rs`: the signed-in branch, with `q`. It needs the
  identity, the designated publication (plan 08's `own_publication`),
  a listing or a search, tags, and the hosted host for the address.
- `read.rs` and a new `search.rs` as above.
- `eaten-at-web/src/components.rs`: nothing new; `listing`, `tag_links`,
  and the pills are reused. `layout::Page` is unchanged.
- `app.css`, `docs/design.md`.
- `scripts/visual-check.mjs`: `landing-signed-in` expects
  `a.button[href="/write"]` and `.listing-item`; `landing-find` visits
  `/?q=noodle` and expects `.listing-item`; `landing-find-none` visits
  `/?q=zzz` and expects `.empty`; `landing-no-publication` with the
  second seeded account expects the "made when you publish" copy.
- Tests: `routes.rs` (signed in: one primary to `/write`, eight items
  from a seed of nine, the ninth reachable through the front page link;
  `?q=` filters and is ignored signed out; settings is a plain link and
  sign-out a POST).

## Definition of done

- Signed in, the first control is "Write a new visit"; the author's
  publication, its recent write-ups, and a way to find one are on the
  page; settings and sign-out are the last line.
- Works with JavaScript off (nothing here uses any).
- `just check` and `just visual-check` pass.

## As built (2026-09-13)

As planned, with these notes.

- The handle sits where the kicker would, but as `.meta`, not `.kicker`:
  a kicker is set in capitals and a handle is not.
- A find shows its whole page of matches (up to twenty), not eight;
  "All write-ups →" is shown only for the recent list, when the
  publication has more than the eight shown.
- The tag chips come from the same page of documents the list does,
  which is how the front page gathers them too.
- Matching lives in `search.rs` beside `tags.rs` and reuses its
  normalization; a query under two characters matches nothing.
- Added after the fact: the find applies itself. A small island fetches
  the same `GET /?q=…` 500 ms after typing pauses and swaps in the
  results section (an `aria-live` region), keeping the address bar in
  step with `replaceState`. It is cheap on the server: after the first
  load a find filters cached document pages in memory, so the cost per
  query is rendering one page. The form still submits as a form.

## Decisions

Settled 2026-09-13:

1. **What "query" means:** a substring find over place name, title,
   and address, scanning the publication as the tag page does, plus
   filtering by tag through the existing tag pages, with the chips
   placed beside the Find form. Not a real index (ruled out by D9).
2. **Eight recent write-ups.** Enough to scroll, short enough that
   "Write a new visit" stays above the fold on a phone.
3. **No account chrome on other pages** in this plan. The feedback is
   about the landing page, and the masthead is one centred name by
   design. Worth revisiting once the editor's entry points are settled.
