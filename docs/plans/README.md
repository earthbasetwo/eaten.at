# Plans from the 2026-09-13 feedback

`docs/feedback.md` collected eleven items. This directory turns ten of
them into seven plans (item 11, a separate new-publication flow, is
deferred and not planned here), each small enough to be one branch and one pull request, each
with the decisions it needs settled before code is written. Nothing here is
implemented yet; the "Decisions" section of each plan lists what is still
open, with a recommendation.

| # | Plan | Feedback items | Size | Depends on |
|---|---|---|---|---|
| 01 | [Editor page head](01-editor-page-head.md) | 1 | trivial | — |
| 02 | [Optional titles](02-optional-titles.md) | 2 | small | — |
| 03 | [Trim the choice lists](03-trim-choice-lists.md) | 7, 8 | small | — |
| 04 | [Remove the cover image](04-remove-cover-image.md) | 9 | small | — |
| 05 | [Place identity: one GERS id](05-place-identity.md) | 4, 5 (model) | medium | — |
| 06 | [Place search with Open Places](06-place-search.md) | 3, 5, 6 | large | 05 |
| 07 | [Photos](07-photos.md) | 10 | large | 04 (recommended) |

## Order

Plans 01–04 all touch the editor (`crates/eaten-at/src/editor/`)
and are independent of one another, but doing them first, in numeric
order, means the two large editor plans (06, 07) start from a leaner
form and fewer tests need rewriting twice. Suggested sequence:

```
01 → 02 → 03 → 04 → 05 → 06 → 07
```

05 must precede 06 (06 fills the field 05 defines). 04 should precede 07
(07 replaces what 04 removes; doing them the other way round means
carrying two image models at once).

## Working agreement for every plan

- One branch per plan, one pull request, merged before the next starts
  unless the two are independent and touch different files.
- `just check` green: formatting, clippy with warnings denied, tests.
- `just visual-check` green for any plan that changes a page, with the
  script extended to cover any new page or state.
- Lexicon changes ship with `lexicons/*.json`, `docs/lexicons.md`, the
  seed in `scripts/dev-env.mjs`, and a note that `just lexicons-check`
  will report drift until `just lexicons-publish` is run. Pre-launch:
  there are no records to migrate and no compatibility to keep.
- Every plan that settles a question adds a row to `docs/decisions.md`
  (numbered from D29) and, where it changes how a page looks, updates
  `docs/design.md`.
- `insta` snapshots are reviewed by hand, never blindly accepted.
- Foreign records stay honoured: a value another client wrote is
  preserved on rewrite even where our editor no longer offers it.
