# Composer refactor — September 22 feedback

Working branch: `composer-refactor`. This is an iterative implementation for
side-by-side review, following Ken's September 20 feedback and September 22
clarifications.

## Checklist

Checked items are implemented on the local branch, not yet committed or merged.

- [x] Keep the restaurant headline stable; use Margot’s Bistro as the example and a
  underlined name that opens the restaurant chooser. The address is plain text.
- [x] Separate Title from the restaurant, inside DIGEST. Remove the full-width
  underline and optional-text hint; retain a text underline on hover/focus.
- [x] Use page-colored paper and faint horizontal rules around title and body.
- [x] Add the italic stone prompt: “What did you eat? Was it good? What else happened?” It is never saved as content.
- [x] Put Formatting beside DIGEST, with examples in a disclosure.
- [x] Remove the Formatting disclosure and the DIGEST kicker. `For [a meal] on
  [date].` is the digest's head on its rule, and the prompt, written in markdown,
  shows one bold word between faint marks; that is the whole discovery. A pilcrow
  that disclosed the marks was built and cut the same day (2026-09-25).
- [x] Preserve active-line-only markdown while supporting whole-digest Select
  All, copy, cut, replacement, paste, undo, and redo.
- [x] Collapse whole-digest selections with navigation keys before typing.
- [x] Restore restaurant details and identity together with drafts; refresh
  the visible name/address and flag older drafts missing this information.
- [x] Move Filed under below photos, full-width and left-aligned.
- [x] Left-align Elsewhere links and their editing cards; allow wrapping.
- [x] Remove the Photos kicker; show Add photos without hovering.
- [x] Remove Cancel and the composer’s Bluesky toggle/text input; stop carrying
  crosspost defaults through the chooser.
- [x] Start the digest at six lines and let longer posts grow. Check a short
  draft with photos at 1440 × 800; this may scroll with the larger writing area.
- [x] Tighten lower-page spacing, including the extra padding inherited by the
  empty photo grid.
- [x] Move the meal selector beside the date: “For a meal on [date],” with
  lowercase meal names and “a snack” when selected.
- [x] Add Snack while retaining Late night and foreign meal values.
- [x] Fix previews of newly uploaded, unpublished photos. Cache processed
  renditions privately by author and CID, replacing prior cached misses.
- [x] Enlarge the caption lightbox up to 960px where the viewport allows it;
  adapt image height to the window, keep controls reachable, retain its shadow.
- [x] Remove the redundant COVER badge; retain the first-photo explanation.
- [x] Keep keyboard focus inside the photo lightbox and return it on close or
  removal; fix Done/backdrop clicks discarding caption edits.
- [x] Preserve the calendar, rating/price controls, teaser disclosure, link
  editing, and inline Delete confirmation.

- [x] Change the restaurant or correct its address through the chooser.
  Select the full existing name on arrival so typing replaces it; use “Keep
  writing” when returning to a draft and “Start writing” for an initial choice.
- [x] Keep one “Describe this photo” field. Show it as a caption on the digest,
  and as alt text where the photo appears without its caption.

## Still to consider

- [ ] Try the agreed title/body prompt with real writing.
- [ ] Test place selection with duplicate names, multiple branches, long
  addresses, distant restaurants, and no location/history. The local stub
  cannot establish real-world search quality.
- [ ] Improve recovery for long-lived photo drafts: unpublished preview caches
  currently expire after six hours, after which a photo needs re-uploading
  unless its saved record makes the blob available from the PDS.
- [x] Trial a just-published / changes-saved confirmation with copy-permalink
  and Bluesky intent sharing (2026-09-22). Thread discovery remains deferred.
- [ ] Settle how a Bluesky post becomes the comments thread, including multiple
  posts or accounts. Automatic discovery is explicitly deferred.
- [ ] Publish the updated Snack lexicon when ready. The local schema, Rust
  vocabulary, docs, and seed are updated; remote lexicon publication is separate.
- [ ] Review the branch, then commit and open a PR when the iteration is ready.

## Upload verification

A PDS may accept a blob but refuse to serve it before a record references it.
The app now caches private previews from the processed JPEG. Responses require
sign-in and vary by Cookie. A regression test models this PDS behavior, checks
thumbnail/full/card previews, prior-miss recovery, author isolation, and saving
the blob with a record. A generated PNG was also uploaded against the real local
PDS: upload and both previews returned 200 while the PDS refused the unpublished
blob. This does not establish support for every source image format.

## Verification

Run `just check` and `just visual-check`. Browser coverage includes whole-digest
keyboard selection, clipboard content, replacement, undo/redo, formatting help,
the empty-body prompt, and wrapped tags at desktop and phone widths. Photo
lightbox coverage includes wide and short windows, focus trapping/return,
caption persistence, and photo removal. Inspect screenshots manually.
