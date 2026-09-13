# 07 · Photos

Feedback 10: *Build an "Add photos" flow that lets a user attach as many
photos as they want to a visit. This replaces the cover image.*

Recommended after plan 04, so the cover is already gone from the editor
and the pages, and this plan only adds.

## Record

`lexicons/at.eaten.visit.json`:

- `photos`: array of `#photo`, `maxLength` 24.
- `#photo`: `image` (blob, `accept: ["image/*"]`, `maxSize: 1000000`,
  required); `alt` (string, ≤ 2000 bytes / 1000 graphemes, optional);
  `aspectRatio` (`{width, height}` integers, optional: lets the page
  reserve space before the image loads, as Bluesky does).

Photos are the visit's, so they live in our content object, not in the
Standard document. The document's `coverImage` is then *derived*: the
first photo's blob ref is written there too, so Standard
readers and unfurlers get a thumbnail and the Bluesky link card reuses
the blob.

Rust: `Visit.photos: Vec<Photo>` with lenient decoding (a malformed
entry from another client is dropped with a debug log, not the visit).

## The flow

Its own page, after the write-up exists, like the crosspost page:

- `GET /write/{rkey}/photos`: kicker "Photos", `h1` "Photos of {place}",
  the current photos as a grid (thumbnail, an `alt` text field, "Move
  up", "Move down", "Remove" as link buttons), then one `file` input
  with `multiple` and a primary "Add photos". Under it, secondary
  "← Back to the write-up".
- `POST /write/{rkey}/photos` (multipart), actions:
  - `add`: each file is checked and re-encoded (below), uploaded with
    `session.upload_blob`, appended; then the document is rewritten
    once with the new list and `coverImage`. Files that fail are
    reported by name beside the input; the good ones still go in.
  - `save`: writes the `alt` texts.
  - `remove:{i}`, `up:{i}`, `down:{i}`: reorder or drop, then write.
  Every action that changes the record writes it at once and re-renders
  the page; nothing is staged, so there is no hidden state to lose.
- Entry points: after a first publish, the editor redirects to
  `/write/{rkey}/photos?new=1`, whose lede says "Published. Add photos
  now, or skip." with a secondary "Skip for now" that continues to the
  crosspost handover or the document, whichever `after_publish` would
  have chosen. The edit page's actions row gains a secondary "Photos"
  link. The document footer's quiet links gain "photos" beside "edit"
  for the author.

Why not in the editor: a JavaScript-free form re-posts itself for every
action (preview, add a row, search), and a `file` input does not survive
that round trip; today's cover already has this flaw. Uploading on a
page whose every action writes the record has no such gap.

## Processing an upload

In `img.rs`, `photo_upload(bytes) -> Result<Photo, ImageError>`:

- Reject files over 10 MB before decoding; decode with the existing
  dimension and allocation limits; raster formats only (JPEG, PNG, GIF,
  WebP), sniffed, never trusted from the declared type.
- Apply EXIF orientation (`image` 0.25.10: `ImageDecoder::orientation`
  and `DynamicImage::apply_orientation`), then **always** re-encode as
  JPEG: long side ≤ 2048, quality 84, stepping down (1600, 1200 / 72,
  60) until ≤ 1,000,000 bytes, as `cover_upload` did. Re-encoding strips
  every metadata block, so a phone photo's GPS position never reaches
  the author's public repository. Say so on the page.
- Record `aspectRatio` from the encoded dimensions.
- The photos route's body cap is its own: 64 MB, at most 12 files per
  request (checked before any upload). Processing runs under
  `spawn_blocking`, one file at a time.

## Serving

Extend the proxy: `/img/{did}/{doc}/{cid}?size=thumb|full`. The handler
loads the document, checks that `cid` names one of its photos (never an
open blob proxy), fetches and decodes the blob, and serves a fresh JPEG:
`thumb` is a 400 px centre-cropped square, `full` fits 1600 px. Cached
in `Namespace::Image` by `did/rkey/cid/size`. `/img/{did}/{doc}` without
a cid (card and OG sizes) now sources the first photo, then a foreign
`coverImage`, then the placeholder, so `og:image`, the feed enclosure,
and the link card follow automatically.

## Rendering

- Document page: a photo grid (`section.photos`, `aria-label`
  "Photos") between the visit card and the prose: `thumb` renditions in
  a responsive grid, each a link to its `full` rendition, `alt` as
  written (empty `alt` stays empty). Nothing is rendered for a visit
  without photos.
- Listing card: the first photo's `thumb` at the left, where the cover
  used to be (this reinstates the `--cover-small` slot plan 04 removed;
  note it in the same design.md section).
- Visit card: no image.
- Preview in the editor: unchanged (photos are not part of the draft).

## Touchpoints

- `lexicons/at.eaten.visit.json`, `docs/lexicons.md`, `at_eaten.rs`
  (`Photo`, `Visit.photos`), `scripts/dev-env.mjs` (upload two small
  PNGs to the local PDS for one seeded visit; the runner already talks
  to the PDS).
- New `routes/photos.rs` and `editor/photos.rs` (form, actions, view);
  `app.rs` routes and body cap; `routes/write.rs` `after_publish`;
  `routes/document.rs` footer and grid; `routes/image.rs` and `img.rs`;
  `paths.rs` (`photo(did, rkey, cid, size)`); `publish.rs`
  (`build_document` writes `coverImage` from the first photo;
  `link_card` needs no change); `view.rs`, `components.rs`, `app.css`.
- `auth/mod.rs`: nothing; `blob:image/*` and
  `repo:site.standard.document` are already granted.
- `docs/design.md`: Photos page, photo grid component, listing card.
- `docs/decisions.md`: D37 photos in the visit; D38 derived
  `coverImage`; D39 always re-encode (metadata stripped).
- README routes table.
- Tests: `img.rs` (orientation applied, size stepping, metadata gone:
  assert the output has no APP1 segment); `routes.rs` with `wiremock`
  for the upload-then-write sequence, a failing file among good ones,
  remove/reorder/alt, the 12-file and 64 MB limits, the proxy refusing
  a cid the document does not list; a snapshot of the grid; visual
  check pages `photos-empty`, `photos`, and the document page with
  photos.

## Definition of done

- An author can add, caption, reorder, and remove photos on a published
  write-up without JavaScript; each change is one record write.
- The document page, listings, `og:image`, the feed, and the Bluesky
  card all show the first photo; the grid shows them all.
- Uploaded photos carry no EXIF; the blob is ≤ 1 MB; orientation is
  right.
- `just check` and `just visual-check` pass.

## As built (2026-09-13)

As planned, with two notes. The inline crosspost still happens at
publish time, before the photos page, so a card posted then carries the
placeholder thumbnail rather than a photo; the crosspost page reached
later (no permission yet, or a retry) does see the photos. And "Save alt
text" is its own action, but the alt texts as typed are applied to every
action, so a caption written before "Move up" is not lost.

## Decisions

Settled 2026-09-13: a cap of 24 photos; `coverImage` derived from the
first photo; the grid between the visit card and the prose with no image
on the card; `alt` text only; square 400 px thumbnails.
