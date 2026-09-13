# Decisions inherited from album-report

eaten.at was forked from album-report at commit `88bcfa4` on 2026-09-12.
Code comments that cite "plan §n" or a decision number "Dn" refer to that
repository's `album-report-plan.md`, which is not copied here. This file
carries the decisions that transfer, so the citations still resolve to
something.

## Settled decisions

| # | Decision | Choice |
|---|---|---|
| D1 | Deployment model | Open write (any atproto identity), publication-scoped read. No global feed. |
| D2 | Web tier | Rust: `axum` + typed HTML templates (`maud`). Server-rendered. |
| D3 | Client JS | Hard budget. Progressive enhancement only; every page works with JS off. |
| D4 | Ratings | Settled 2026-09-13: an optional four-step house scale stored as the integer 1–4 in `at.eaten.visit.rating`, rendered as plus signs with a word: Solid (+), Recommended (++), Strongly Recommended (+++), Can't Miss (++++). An integer with a range, not open `knownValues`: the scale is ordinal and ours. |
| D5 | Subscriptions | Deferred. The follow story is RSS. |
| D6 | Comments | Off-platform via `bskyPostRef` to a Bluesky thread. No native comments. |
| D7 | Recommends | Out of scope (`site.standard.graph.recommend`). |
| D8 | Publication hosting | User's choice: a subdomain on `eaten.at`, or bring your own domain. |
| D9 | Indexing | No content index. Read-through from PDSes with a TTL cache. |
| D10 | Routing | Publication-scoped. `/at/<did>/` is the default publication (or a chooser); `/at/<did>/<rkey>/` a specific one. Users may have many. |
| D11 | Subject identity | Settled 2026-09-13: a place is matched across write-ups by external ids as `knownValues` (`googlePlace`, `applePlace`, `overtureGers`) in `at.eaten.place.ids`. Two places sharing a `(service, id)` pair are the same place; names and addresses are for people. Unknown services are preserved. |
| D12 | Body | `at.markpub.markdown` in the `content` union. `text.markdown` only: no facets, lenses, or rendering rules. |
| D13 | Internal crates | Two workspace crates, `eaten-at-web` and `eaten-at-atproto`. Local path deps, not published. |
| D14 | OAuth scopes | Granular scopes requested directly. No custom permission set. |
| D15 | NSID shape | Flat, single authority: `at.eaten.<name>`. One DNS record (`_lexicon.eaten.at`), one repo. |
| D16 | Datastore | SQLite. Single node, tiny write volume, one-file backup, matches the single-binary shape. |
| D17 | Bluesky crosspost | Opt-in and separately authorized. Creates the post that comments thread from. |
| D18 | Tags | Free text, entirely the author's own. No suggested vocabulary. Publication-scoped tag pages. Reaffirmed as D26. |
| D19 | Excerpt | `description` stores only what the author wrote. Summaries derived at point of use, never persisted. |
| D20 | Site-route addressing | DID-addressed, not handle-addressed. Handles are lookup input only, resolved to a redirect. |
| D21 | Document rendering | Site routes render documents in full, with `rel="canonical"` to the publication origin. |
| D22 | Meta / unfurling | Full OpenGraph set; `og:url` and canonical are always the publication origin. `og:image` is one stable proxy URL per document. |

## Decisions made for eaten.at

Taken on 2026-09-13, when the placeholder subject became a visit.

| # | Decision | Choice |
|---|---|---|
| D23 | Where our model lives | Our object is the document's **`content`**, not an entry in `links`. A write-up stays a `site.standard.document`, so publications, hosting, theming, canonical URLs, covers, tags, labels, and the Bluesky thread stay on the Standard record; `content.$type == at.eaten.visit` is what marks a document as ours. `textContent` carries a plaintext rendering (place, date, verdict, prose, dishes) for readers that do not know the type. The earlier "lens" idea, that a write-up should read as a plain post everywhere, was dropped; `links` is not ours and foreign entries there are carried over untouched. |
| D24 | The subject is a visit | `at.eaten.visit`: a place, a calendar date, an optional meal, the dishes, an optional rating, and the prose as an open-union `body` (we write `at.markpub.markdown`, so D12 stands). |
| D25 | Place fields | `at.eaten.place`, its own lexicon of object defs so later records can reference it: name, one-line address, a 1–4 price band, external ids, and links for readers. No cuisine field; cuisine is a tag if the author wants one. |
| D26 | Tags | Free-text tags stay in `site.standard.document.tags` (D18 stands). The visit carries typed facts, never tags: add meaning only where Standard leaves a slot open, never shadow a field Standard already has. |
| D27 | Visit date | `visitedOn` is a calendar date string `YYYY-MM-DD`, not an atproto `datetime`: a visit has no instant, and the server-rendered editor cannot know the author's offset. The PDS checks the length; the app checks the format. |
| D28 | Company | No party size or companions in the record. Naming other people in a public record is left to the prose. |

## Stack choices

| Concern | Chosen | Alternative | Why |
|---|---|---|---|
| SQLite driver | `rusqlite` (bundled) behind `spawn_blocking` | `sqlx` | Write volume is tiny; no build-time DB requirement; the single-connection shape suits a one-file DB. |
| XRPC/record types | Hand-rolled `serde` structs for the handful of endpoints used | `atrium-api` generated types | Tolerance of unknown fields and the `links` singular-or-array quirk; a few hundred lines fully under our control. `atrium-oauth` is used for OAuth, behind our own facade (`docs/oauth-eval.md`). |
| DNS | `hickory-resolver` | `trust-dns` | Maintained, async, TXT lookups for `_atproto` and `_lexicon`. |
| HTTP mocking in tests | `wiremock` | `httpmock` | Async-native, pairs with reqwest/tokio. |
| Snapshot tests for HTML | `insta` | substring asserts | Rendered maud output is stable; snapshots catch regressions in meta tags and structure cheaply. |
| Image re-encoding | `image` crate to JPEG | `libvips` bindings | Pure Rust, no system deps, good enough for cover-sized images. |

## Decided for eaten.at

| Date | Decision | Choice |
|---|---|---|
| 2026-09-12 | Visual design | The Campari design system (`docs/design-handoff/campari/`) replaces album-report's "paper journal" direction. `docs/design.md` has the rules. |
| 2026-09-12 | Publication themes under Campari | Kept. Campari is the default palette; an author's four colors replace ground, ink, and accent, and the other Campari tokens are derived from them (`theme.rs` for the raised and sunken surfaces, `color-mix` for ink shades and borders). |
| 2026-09-12 | Dark mode | None. Campari is a light palette and the site stays light whatever the system preference. |
| 2026-09-12 | Listings | Cards (raised surface, border, 12px radius), not hairlined rows. |
| 2026-09-12 | Prose size | Long-form prose is DM Sans at 17→18px; the rest of the interface uses Campari's exact 14–15px body scale. |
| 2026-09-12 | Fonts | Instrument Serif, DM Sans, and JetBrains Mono are self-hosted woff2 subsets, not loaded from Google Fonts, so the CSP keeps every request same-origin. |

## Left over from the fork

Cosmetic only; nothing here affects behaviour.

- Code comments still cite plan sections and decision numbers (see the
  first paragraph).
- The markdown fixture `crates/eaten-at-web/tests/fixtures/write-up.md`
  and its snapshots still read as an album write-up. They exercise the
  markdown pipeline, not the subject.
- Rust identifiers were renamed from "subject document" and "subject
  card" to "visit document" and "visit card" on 2026-09-13, when the
  vocabulary settled.
