# Plans from the 2026-09-13 feedback

Two rounds: the first (01–07) from the written feedback, the second
(08–13) from walking through the UI afterwards. The second round's
section is at the end of this file.

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

## Status

All seven were implemented on 2026-09-13, one commit each on `main`
(`bb495c1` through `e0b7826`), with the decisions they settled recorded
as D29–D39 in `docs/decisions.md`. Each plan file ends with what was
settled and, where the build departed from the plan, an "As built" note.
Not yet done: publishing the changed lexicons (`just lexicons-publish`),
which `just lexicons-check` will keep pointing out.

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

## Second round (2026-09-13, after walking through the UI)

`docs/feedback.md` collected eleven more items (12–22). Six plans, same
working agreement as above. Plans 08–12 were implemented on
2026-09-13 (D40–D45); 13 is not yet. Each plan's "Decisions" section records what
was settled.

| # | Plan | Feedback items | Size | Depends on |
|---|---|---|---|---|
| 08 | [One publication per account](08-one-publication.md) | 12, 16 (part), and 11 from the first round | large | — |
| 09 | [Landing page, signed out](09-landing-signed-out.md) | 13 | small | — |
| 10 | [Handle autocomplete on sign-in](10-handle-typeahead.md) | 17, 18 | medium | 09 |
| 11 | [Landing page, signed in](11-landing-signed-in.md) | 14, 15, 16 | medium | 08, 09 |
| 12 | [A new visit starts with the place](12-place-first.md) | 19, 20, 21 | large | 08, 10 |
| 13 | [Listing cards that show the photos](13-listing-cards.md) | 22 | medium | — |

### Order

```
08 → 09 → 10 → 11 → 12 → 13
```

08 first: it removes the publication fields from the editor, which 12
rewrites, and defines the designated publication that 11 shows. 09
before 10 because 10 changes the lookup field 09 moves. 10 before 12
because 12 reuses the combobox 10 builds. 13 is independent and can go
anywhere; doing it before 11 means the landing page's list is right the
first time, doing it last keeps the round's biggest visual change on
its own branch.

### What the round settles as a whole

- Every decision in plans 08–13 is settled as of 2026-09-13; each
  plan's "Decisions" section records what was chosen and why. One
  thing is deliberately left open: the canonical word for a document.
  "Write-up" is not it, and the replacement is not chosen; the plans
  use the word they found, and the copy changes in one pass later.

- The JavaScript budget restated as a rule, be judicious, with one
  20 KB tripwire and no per-page cap (D43 as amended), and two more
  functions that script makes quicker: handle suggestions from the Bluesky
  AppView (10) and place suggestions through our server (12). The
  browser geolocation island from plan 06 goes; the point comes from
  the request's IP (12).
- The CSP stays strict everywhere; the sign-in and landing pages alone
  allow `connect-src` to the configured AppView (10).
- `at.eaten.preferences.defaultPublication` becomes the designation of
  the account's one publication (08), which is another reason the
  pending `just lexicons-publish` should wait until this round is in.
