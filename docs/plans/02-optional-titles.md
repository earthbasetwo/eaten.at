# 02 · Optional titles

Feedback 2: *Titles should be optional. The name of the place visited is
the most important thing and should be the default title, unless the
user chooses to give the visit its own title.*

## Constraint

`site.standard.document.title` is required by Standard. A document
without a title is not a valid document, so a title is always written.
"Optional" means the author need not type one: when the field is blank,
the place's name is written as the title.

## Design

**Form.** The title field becomes "Title (optional)" with the place's
name as its placeholder and the hint "Left blank, the place's name is the
title." (After plan 06 the place is known before the form is shown, so
the placeholder is always real; until then it is whatever is in
`place_name`, which for a blank form is empty.)

**Prefill on edit.** `EditorForm::from_document` leaves the title blank
when `doc.title.trim() == visit.place.name.trim()`. An author who
deliberately titled a write-up with the place's name is indistinguishable
from one who did not; the result is the same either way.

**Validation.** `draft::validate` drops "Give the write-up a title." and
resolves `title = typed title, else place name`. `DocumentDraft.title`
stays a `String`: everything downstream (`build_document`, the path
slug, the preview) is unchanged. The length cap stays.

**Path.** The slug comes from the resolved title, so an untitled visit
gets `/2026/09/devocion`; a second visit to the same place in the same
month gets `-2`. That is acceptable and needs no change to
`publish::document_path`.

**Rendering.** `view::document_headline` today returns the place name
for every visit regardless of the title, which is used for `og:title`,
the feed item title, and the Bluesky link card. With titles defaulting
to the place name, the headline becomes simply the document's title.
Delete `document_headline` and use `title` at its three call sites
(`view::document_meta`, `routes/feed.rs`, `publish::link_card`).

**Duplication.** When the title is the place's name, the document page
shows the name twice (the `h1` and the visit card) and a listing card
shows it twice (title line and `.listing-place` line). See decision 1.

## Touchpoints

- `editor/form.rs`: `from_document`.
- `editor/draft.rs`: `validate`, tests `every_problem_is_reported_beside_its_field`
  (title error goes), a new test for the default.
- `editor/view.rs`: the title field's label, placeholder, hint;
  `preview` unchanged.
- `view.rs`: `document_headline` removed; `listing_item` gains the
  collapse rule if decision 1 chooses it.
- `eaten-at-web/src/components.rs`: `ListingItem.place_name` becomes
  `Option<String>` if the listing collapses; `listing()` renders the
  rating alone on that line when the name is absent.
- `routes/feed.rs`, `publish.rs` (`link_card`): use the title.
- `docs/lexicons.md`, "What we write into the Standard document": `title`
  is "the author's, or the place's name".
- `docs/design.md`: the listing card description.
- `docs/decisions.md`: D29 "Titles are optional; the place's name is the
  default title and is what is written to `title`."
- Tests: `routes.rs` editor tests posting a blank title now succeed;
  `document_page_renders_card_body_tags_and_canonical` and the head tests
  check `og:title`; the feed test checks the item title.

## Definition of done

- A blank title publishes with `title` equal to the place's name.
- Editing such a document shows a blank title field with the name as the
  placeholder; saving without typing keeps the title in step with the
  (possibly edited) place name.
- A typed title is used everywhere the headline appears.
- `just check`, `just visual-check` pass.

## Decisions

Settled 2026-09-13: listing cards drop the place line when it equals the
title; the document page keeps both. A blank title tracks the place
name on every save.
