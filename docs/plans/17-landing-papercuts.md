# 17 · Landing page papercuts

Working branch: `landing`, from `places` (PR #4). The Backlog's two
landing sections on 2026-09-27, as Ken sorted them: L1 (the pitch's
copy) waits for Ken and Ross; L4 (a way in without a handle) is set
aside; the rest are here.

## Signed out

- **L3, the divider pushed down.** Measured before the fix: pressing
  "Connect to start writing" grew the block from the button's 23px to
  the field's 38px, and the hairline and everything below moved 15px.
  Now the block is a grid whose hidden form (`visibility: hidden`, not
  `display: none`) still sizes it, and the button stands at the bottom,
  on the line the field's rule will take. The moving rule only widens;
  nothing below moves. Without script the button sits 15px lower than
  it did, which is the whole cost.
- **L7, a favicon.** The logotype's e (Ken, 2026-09-26), as
  `static/icons/favicon.svg` with a dark variant, a 32px PNG and the
  180px apple-touch-icon; linked from every page's head and served at
  the two bare paths as well. How it was made is in `docs/design.md`
  under "Icon".

## Signed in

- **S3, no Find button.** The field is sent with Return and wears the
  return mark, as the connect fields do. The live find filters anyway.
- **S1, fewer type styles.** The "Find a digest" kicker, the one dark
  kicker on the page, is a hidden label now; the placeholder says "Find
  a place, a title, a street". With the italic button gone the page has
  one kicker style, and the rest each carry different information: the
  handle, the h1, the primary action, the nameplate, the field, the
  chips, the rows.
- **S4, the tag row.** Each chip carries its count, faint, and the row
  runs most-used first; with script it folds to its first line behind
  "+n more" (`find.js`), refolding on resize until unfolded.
- **S5, the rule under "Recent digests".** A hairline on the home; the
  kicker opens the list. Elsewhere the listing keeps its ink rule.
- **S6, a long title and the rating.** Checked with an 88-character
  title at 1024 and 390: the head wraps and the verdict drops under the
  title, left-aligned, on its own line. Nothing crowds. Whether the
  verdict should always sit under the title, for one shape on every
  row, is Ken's call; the row stays open for that.
