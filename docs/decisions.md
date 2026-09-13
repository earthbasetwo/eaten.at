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
| D4 | Ratings | **Decide for eaten.at.** album-report had none; a review site may want them. |
| D5 | Subscriptions | Deferred. The follow story is RSS. |
| D6 | Comments | Off-platform via `bskyPostRef` to a Bluesky thread. No native comments. |
| D7 | Recommends | Out of scope (`site.standard.graph.recommend`). |
| D8 | Publication hosting | User's choice: a subdomain on `eaten.at`, or bring your own domain. |
| D9 | Indexing | No content index. Read-through from PDSes with a TTL cache. |
| D10 | Routing | Publication-scoped. `/at/<did>/` is the default publication (or a chooser); `/at/<did>/<rkey>/` a specific one. Users may have many. |
| D11 | Subject identity | **Decide for eaten.at.** album-report used an optional MusicBrainz release-group id; the placeholder subject here has only a title and links. |
| D12 | Body | `at.markpub.markdown` in the `content` union. `text.markdown` only: no facets, lenses, or rendering rules. |
| D13 | Internal crates | Two workspace crates, `eaten-at-web` and `eaten-at-atproto`. Local path deps, not published. |
| D14 | OAuth scopes | Granular scopes requested directly. No custom permission set. |
| D15 | NSID shape | Flat, single authority: `at.eaten.<name>`. One DNS record (`_lexicon.eaten.at`), one repo. |
| D16 | Datastore | SQLite. Single node, tiny write volume, one-file backup, matches the single-binary shape. |
| D17 | Bluesky crosspost | Opt-in and separately authorized. Creates the post that comments thread from. |
| D18 | Tags | Free text, entirely the author's own. No suggested vocabulary. Publication-scoped tag pages. |
| D19 | Excerpt | `description` stores only what the author wrote. Summaries derived at point of use, never persisted. |
| D20 | Site-route addressing | DID-addressed, not handle-addressed. Handles are lookup input only, resolved to a redirect. |
| D21 | Document rendering | Site routes render documents in full, with `rel="canonical"` to the publication origin. |
| D22 | Meta / unfurling | Full OpenGraph set; `og:url` and canonical are always the publication origin. `og:image` is one stable proxy URL per document. |

## Stack choices

| Concern | Chosen | Alternative | Why |
|---|---|---|---|
| SQLite driver | `rusqlite` (bundled) behind `spawn_blocking` | `sqlx` | Write volume is tiny; no build-time DB requirement; the single-connection shape suits a one-file DB. |
| XRPC/record types | Hand-rolled `serde` structs for the handful of endpoints used | `atrium-api` generated types | Tolerance of unknown fields and the `links` singular-or-array quirk; a few hundred lines fully under our control. `atrium-oauth` is used for OAuth, behind our own facade (`docs/oauth-eval.md`). |
| DNS | `hickory-resolver` | `trust-dns` | Maintained, async, TXT lookups for `_atproto` and `_lexicon`. |
| HTTP mocking in tests | `wiremock` | `httpmock` | Async-native, pairs with reqwest/tokio. |
| Snapshot tests for HTML | `insta` | substring asserts | Rendered maud output is stable; snapshots catch regressions in meta tags and structure cheaply. |
| Image re-encoding | `image` crate to JPEG | `libvips` bindings | Pure Rust, no system deps, good enough for cover-sized images. |

## Left over from the fork

Cosmetic only; nothing here affects behaviour.

- Code comments still cite plan sections and decision numbers (see the
  first paragraph).
- The markdown fixture `crates/eaten-at-web/tests/fixtures/write-up.md`
  and its snapshots still read as an album write-up. They exercise the
  markdown pipeline, not the subject.
- Rust identifiers use "subject document" and "subject card" for what
  album-report called the album document and album card; rename again
  when eaten.at's own vocabulary settles.
