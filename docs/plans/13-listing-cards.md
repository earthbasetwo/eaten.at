# 13 · Listing cards that show the photos

Feedback 22: *The publication view's cards (the list of documents in a
publication) should highlight the images included in a visit, if any,
and look elegant whether or not there are images.*

Medium. Independent of the rest; plan 11 lists the same cards on the
landing page, so doing this before 11 lands means the landing page is
right the first time, but either order works.

## The card today

A raised card with the first photo as a small square thumbnail
(`--thumb-small`, 72→96px) at the left, and the kicker date, title,
place line, and excerpt at the right. Without a photo the left slot is
simply absent. The thumbnail is a token, not a highlight: at 96px a
photo of a dish reads as an icon.

## The card after

Two variants of one component, sharing the text block exactly.

**With photos** (`.listing-item.has-photos`): the first photo leads.

- Under 40rem: the photo spans the card's width at the top, cropped to
  3:2, corners following the card's radius; the text block beneath with
  the card padding.
- From 40rem: the photo fills the left 42% of the card edge to edge,
  cropped to fill the card's height (at least 3:2 of its width), text
  block at the right. The card is a two-column grid, not a flex row, so
  the photo can bleed to the card's edges while the text keeps its
  padding.
- More than one photo: a small mono count at the photo's bottom-right
  corner on a translucent raised chip, "+3 photos" (the count of the
  rest). No strip of thumbnails; one strong image is the highlight, and
  the document page has the grid.
- `alt` is the first photo's own alt text; empty stays empty. The whole
  photo is inside the title's link, so the card has one link target for
  the image and the title (and the place line stays plain text).
- `width` and `height` come from the photo's `aspectRatio` when the
  record has it, so the card reserves its space before the image loads;
  without one, the 3:2 default.

**Without photos** (`.listing-item.no-photos`): the text block alone,
with the card padding on all sides, as today minus the thumbnail slot.
So that a text-only card does not look like something is missing, the
kicker date moves up to sit on the card's top edge as a small mono line
and the title gets the room: the same three lines as the photo card's
text block, only wider. Nothing decorative stands in for the photo (no
placeholder, no generated image); a listing of text-only cards reads as
a plain journal, which is what it is.

Mixed listings alternate between the two without any alignment tricks;
both variants have the same corner radius, border, shadow, and gap, so
the column stays even.

### The rendition

The photo proxy serves `thumb` (400px square) and `full` (1600px).
Neither suits the card: the square crops a landscape dish photo hard,
and full is heavy for a list. Add `card`: 960px on the long side,
cropped to 3:2, JPEG at the usual quality, cached in `Namespace::Image`
like the others; `PhotoSize::Card`, `?size=card`. The landing page's
list of eight and a publication page of twenty would each load at most
twenty such images, lazily.

### View model

`ListingItem` gains `photo: Option<ListingPhoto>` replacing `thumb_src`,
where `ListingPhoto { src, alt, width, height, more: usize }`. `view::listing_item` fills
it from the visit's photos and `aspectRatio`. The feed and the OG image
are untouched (they use the cover rendition).

## Design

- `docs/design.md`: the **Listing card** entry is rewritten as above,
  with both variants and the count chip; the `--thumb-small` slot goes
  from the listing (it stays on the photos page's rows). Principle 4
  ("cards on blush") gets one sentence: a card may bleed one image to
  its edge, and only an image.
- `app.css`: `.listing-item article` becomes a grid with the two
  variants, `.listing-photo` (the image, `object-fit: cover`,
  `aspect-ratio: 3 / 2` under 40rem), `.listing-photo-count` (the
  chip: mono, `--color-raised` at 85%, `--radius-s`, 4px inset).
  Publication themes recolour the chip through the same tokens.

## Touchpoints

- `eaten-at-web/src/components.rs` (`ListingItem`, `ListingPhoto`,
  `listing`), `view.rs` (`listing_item`), `paths.rs` (`photo` already
  takes a size string).
- `img.rs` (`PhotoSize::Card` and its crop), `routes/image.rs`
  (`?size=card` accepted), the proxy's tests.
- `app.css`, `docs/design.md`.
- `scripts/dev-env.mjs`: the seed already has one visit with two photos
  and others with none; add one with a single photo so the chip's
  absence is visible, and one with a portrait photo so the crop is
  exercised.
- `scripts/visual-check.mjs`: the front-page checks already render the
  listing; add an assertion that `.listing-item.has-photos img` has
  loaded (`naturalWidth > 0`) at both widths, and a screenshot name for
  the landing page's list once plan 11 lands.
- Tests: `components.rs` (both variants' markup; the chip only for
  more than one photo; alt as given; width and height from the aspect
  ratio; no `<img>` for none), `img.rs` (card rendition dimensions for
  landscape, portrait, and square inputs), `routes.rs` (`?size=card`
  serves a JPEG of the right size and still refuses a cid the document
  does not list).

## Definition of done

- A visit with photos leads its card with the first one, large; a visit
  without photos is a clean text card; a mixed listing looks even at
  390px and at desktop width.
- The new rendition is served only for cids the document lists and is
  cached like the others.
- `just check` and `just visual-check` pass.

## As built (2026-09-13)

As planned, with these notes.

- The `card` rendition is the largest 3:2 frame inside the image, at
  most 960 wide, never upscaled: a wide image loses its sides, a tall
  one its top and bottom, a small one is cropped at its own size.
- The card always reserves 960×640 (the crop is always 3:2, whatever
  the source), so the record's `aspectRatio` is not consulted; the
  `ListingPhoto` view model carries the source, the alt text, and the
  count of further photos.
- The photo is its own link to the same page as the title, with
  `tabindex="-1"` so a keyboard user meets one stop per card.
- The visual check now decodes every card photo on every page it
  renders and fails if one does not load, rather than one page's
  assertion.
- The seed's second visit gained a single portrait photo, so a card
  without the count chip and the 3:2 crop of a tall image are both in
  the screenshots; the third visit stays a text card.

## Decisions

Settled 2026-09-13, as recommended:

1. **One image with a count chip**, not a strip. A strip is busier, and
   the document page's grid already does that job.
2. **Crop ratio 3:2**, the common photo shape, so most phone photos
   lose little.
3. **Photo width at desktop: 42% of the card**, about 300px by 200px
   at the 720px column; enough to read a dish while the text keeps a
   comfortable measure. Not a full-bleed top image at every width,
   which makes a twenty-card page very tall.
4. **Nothing stands in for a missing photo.** A generated placeholder
   like the OG image's would make every text-only card look broken.
