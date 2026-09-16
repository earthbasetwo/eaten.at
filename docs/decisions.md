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
| D10 | Routing | Publication-scoped. `/at/<did>/` is the default publication (or a chooser); `/at/<did>/<rkey>/` a specific one. Users may have many (reading side only since D40: eaten.at itself writes to one). |
| D11 | Subject identity | Superseded by D33 on 2026-09-13. Was: a place is matched across write-ups by external ids as `knownValues` in `at.eaten.place.ids`. |
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
| D23 | Where our model lives | Our object is the document's **`content`**, not an entry in `links`. A write-up stays a `site.standard.document`, so publications, hosting, theming, canonical URLs, covers, tags, labels, and the Bluesky thread stay on the Standard record; `content.$type == at.eaten.visit` is what marks a document as ours. `textContent` carries a plaintext rendering (place, date, verdict, prose) for readers that do not know the type. The earlier "lens" idea, that a write-up should read as a plain post everywhere, was dropped; `links` is not ours and foreign entries there are carried over untouched. |
| D24 | The subject is a visit | `at.eaten.visit`: a place, a calendar date, an optional meal, an optional rating, and the prose as an open-union `body` (we write `at.markpub.markdown`, so D12 stands). |
| D25 | Place fields | `at.eaten.place`, its own lexicon of object defs so later records can reference it: name, one-line address, a 1–4 price band, external ids, and links for readers. No cuisine field; cuisine is a tag if the author wants one. |
| D26 | Tags | Free-text tags stay in `site.standard.document.tags` (D18 stands). The visit carries typed facts, never tags: add meaning only where Standard leaves a slot open, never shadow a field Standard already has. |
| D27 | Visit date | `visitedOn` is a calendar date string `YYYY-MM-DD`, not an atproto `datetime`: a visit has no instant, and the server-rendered editor cannot know the author's offset. The PDS checks the length; the app checks the format. |
| D28 | Company | No party size or companions in the record. Naming other people in a public record is left to the prose. |
| D29 | Titles | Settled 2026-09-13: optional in the editor. Standard requires `title`, so a blank one is written as the place's name, and it follows the name on every save; the edit form shows the field blank when the two match. Listing cards leave the place line's name out when it is the title. |
| D30 | Link types | Settled 2026-09-13: one known link service, `officialSite`. Everything else is a plain link with no `service`, labelled by its label or host. `menu` and `reservations` were dropped. |
| D31 | Foreign vocabulary | Settled 2026-09-13: the editor never offers a free-text "Other" for a `knownValues` field. A value another client wrote is shown as its own selected option and preserved on rewrite, but not authored here. |
| D32 | Cover image | Settled 2026-09-13: a write-up has no authored cover. `coverImage` is never written and is carried over like `links`; the image proxy shows a foreign one, else a generated placeholder. Photos replace it (plan 07). |
| D33 | Place identity | Settled 2026-09-13: a place's identity is its Overture Maps GERS id in `at.eaten.place.gersId`; no other id services. `latE6` and `lonE6` carry its position in integer microdegrees, and the only map link is OpenStreetMap at that point. Supersedes D11. |
| D34 | Place search | Settled 2026-09-13: the editor finds places through the Open Places API (Overture data), server-side with a key, results cached for an hour. A place not found can be entered by hand and carries no `gersId`. |
| D35 | Location | Superseded by D44 on 2026-09-13. Was: the search point comes from the browser's geolocation through a small script island, falling back to the author's most recent visit's coordinates, and failing that to manual entry. No geocoder. |
| D36 | Open Places ids | Verified 2026-09-13 against release 2026-08-19: every `place_id` is `overture:` followed by a UUID, the Overture GERS id. The suffix is what `gersId` stores. |
| D37 | Photos | Settled 2026-09-13: photos live in the visit as `at.eaten.visit.photos`, at most 24, each a blob ≤ 1 MB with optional alt text and aspect ratio. They are managed on their own page after publish, where every action writes the record at once; a JavaScript-free editor form cannot carry files across its own re-renders. |
| D38 | Cover from photos | Settled 2026-09-13: `coverImage` is derived, the first photo, so unfurls, the feed, the Bluesky card, and Standard readers get a thumbnail. Removed with the last photo. |
| D39 | Re-encode every upload | Settled 2026-09-13: a photo is decoded (EXIF orientation applied), fitted to 2048 px, and written as a fresh JPEG under the cap. No metadata block survives, so a phone's position never reaches a public repository. |
| D40 | One publication per account | Settled 2026-09-13 (plan 08): an account has one eaten.at publication, the `site.standard.publication` its `at.eaten.preferences.defaultPublication` names. Every write-up is written to it; the editor never asks. An edit keeps its document's `site` as found. Other Standard apps may still add publications to the repo, and the reading side (D10) still resolves a foreign repo through the preference, a lone publication, or the chooser. The field keeps its name so more than one stays possible later. |
| D41 | Publication defaults | Settled 2026-09-13 (plan 08): the publication is created lazily, by the first publish or the first settings save, never at sign-in. Its name is the handle as text (the DID without one); its address a hosted subdomain from the handle's first label, `-2`, `-3`, … appended while taken or reserved, or the publication's own site route where the deployment cannot host. The record key is a TID minted by the app so the address is known before the write. A hosted name is claimed before the write and released if the server refuses. |
| D42 | Handle suggestions | Settled 2026-09-13 (plan 10): the sign-in and landing pages suggest handles as the author types, from the Bluesky AppView's `searchActorsTypeahead`, called from the browser without credentials. The one third-party request a page makes, allowed by those pages' `connect-src` alone. Three characters, 350 ms, five suggestions, display name and handle only (no avatars). The AppView origin is the configured one, so the local network can point it at a stub. |
| D43 | JavaScript budget | Settled 2026-09-13 (plan 10), amended the same day: be judicious with script. No per-page cap; a single 20 KB tripwire on all inline script fails the build, and crossing it is the moment to judge how the site feels, not a reason to trim by itself. Every page still works with none of it, which is the rule the number stands for. |
| D44 | Location by IP | Settled 2026-09-13 (plan 12): the editor's place search looks near where the request's IP is, read from a local database in the MaxMind DB format with the GeoIP2 City layout (DB-IP's IP-to-City Lite, CC BY 4.0, configured by path only), falling back to the author's most recent visit with coordinates, and failing that to manual entry alone. No browser geolocation, no prompt, no third party told the address. The first `X-Forwarded-For` hop is trusted as the proxy's word. Supersedes D35. |
| D45 | Place suggestions | Settled 2026-09-13 (plan 12): the choosing page suggests places as the author types, through this site's own `/write/suggest`, which runs the same cached Open Places search a pick re-reads. Three characters, 300 ms, six suggestions, thirty requests a minute per author. A place can always be entered by hand: a name, and an address if wanted, on the same page. |
| D46 | Rating rendering | Settled 2026-09-14 (Masthead): the verdict is shown as its word alone, in tracked mono capitals in the accent (SOLID, RECOMMENDED, STRONGLY RECOMMENDED, CAN'T MISS). The plus signs of D4's rendering go; D4's scale and storage stand. The handoff's own five words (POOR to SUPERB) were a specimen, not a scale change. |

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
| 2026-09-12 | Fonts | Superseded 2026-09-14. Was: Instrument Serif, DM Sans, and JetBrains Mono are self-hosted woff2 subsets, not loaded from Google Fonts, so the CSP keeps every request same-origin. |
| 2026-09-14 | Visual design | The Masthead design system (`docs/design-handoff/masthead/`) replaces Campari: ivory paper, near-black ink, one vermilion accent; Newsreader and JetBrains Mono, with the Evantic logotype. `docs/design.md` has the rules. |
| 2026-09-14 | Publication themes under Masthead | Kept. Masthead is the default palette; an author's four colors replace paper, ink, and vermilion, and the other tokens are derived from them (`theme.rs` for the bright surface, `color-mix` for the ink shades, stone, and the hairline). The sunken surface is gone: Masthead has two grounds. |
| 2026-09-14 | Dark mode | None. Masthead is a light palette and the site stays light whatever the system preference. |
| 2026-09-14 | Listings | Rows under rules (an ink rule above the list, hairlines between rows), not cards. The visit's fact box on a document page is the one boxed surface. |
| 2026-09-14 | Vermilion contrast | The handoff's vermilion (`#D8401F`) is applied exactly although it reads 3.99:1 on paper and 4.34:1 on paper-bright, under AA for normal text; the handoff calls its colours final. Every use is short tracked capitals, a link that underlines on hover, or a prose link underlined at rest. `docs/design.md` records the numbers; darkening it is a one-token change. |
| 2026-09-14 | Prose size | The write-up's text is Newsreader at 16px, the top of the handoff's 14–16 range, at a 640px measure; the departure to 17–18px made under Campari is dropped. |
| 2026-09-14 | Fonts | Newsreader and JetBrains Mono are self-hosted woff2 subsets, not loaded from Google Fonts, so the CSP keeps every request same-origin. Evantic Regular, the logotype face bundled with the handoff under a personal-use licence, is converted to woff2 and served the same way; it is used for the logotype only. |
| 2026-09-14 | Masthead on publication pages | The logotype and tagline are the site's; a publication's inner pages carry the publication's name as a running head in the serif instead, because Evantic is for `eaten.at` alone. |
| 2026-09-15 | No site header | The site header is gone from every page but the document, which keeps the publication's name as a running head. The signed-out landing page alone carries the logotype, centred at the top of the page rather than in a masthead bar; the tagline is dropped with the bar. Signed in, the author already knows where they are, so their home opens on its own content too. A reader on someone's write-up should see that publication, not this site's chrome. |

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
