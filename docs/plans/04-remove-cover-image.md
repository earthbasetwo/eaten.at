# 04 · Remove the cover image

Feedback 9: *Remove "Cover image". It doesn't fit a restaurant visit.*

Plan 07 replaces it with photos. This plan removes the cover as an
authored thing and as a visual on our pages, and leaves the image proxy
in place for what still needs one image per document (OpenGraph, the RSS
enclosure, the Bluesky link card).

## Design

**Authoring.** The editor's "Cover image" file field, `form::Upload`,
`draft::Cover`, `DocumentDraft.cover`, `MAX_UPLOAD_IMAGE_BYTES`,
`EditorPage.has_cover`, and `img::cover_upload` are deleted. The editor
form no longer carries a file, so it can post as
`application/x-www-form-urlencoded` (decision 1).

**Record.** `build_document` no longer writes `coverImage`. A
`coverImage` on a document we are editing (set by another client) is
carried over untouched, the way `links` is: it is no longer ours.

**Reading.** `VisitCard.cover_src` and `ListingItem.cover_src` go, with
the `.visit-cover` and `.listing-cover` markup and CSS and the
`--cover-size` / `--cover-small` tokens. The document page and listings
show no image.

**The proxy.** `/img/{did}/{doc}` stays, serving the document's
`coverImage` blob if a foreign client set one, else the generated
placeholder, at card and OG sizes. `og:image`, the feed enclosure, and
`publish::link_card`'s thumb keep using it. Plan 07 changes its source
to the first photo.

## Touchpoints

- `editor/{form,draft,view,mod}.rs`; `routes/write.rs` (`Multipart` →
  `Form`, body cap); `app.rs` (`DefaultBodyLimit` for the editor can go
  or drop to something like 1 MB).
- `publish.rs`: `Placement.cover`, the upload step; tests
  `a_new_cover_replaces_the_old_one` (delete),
  `an_edit_keeps_what_the_editor_does_not_know…` (cover is now a
  carried-over field; assert it survives).
- `img.rs`: `cover_upload` and its test go; the rest stays.
- `view.rs`, `eaten-at-web/src/components.rs`, `app.css`.
- `docs/design.md`: Visit card, Listing card, the tokens table, the
  editor's Details group.
- `docs/lexicons.md`: `coverImage` row becomes "not ours; carried over".
- `docs/decisions.md`: D32.
- Tests: `routes.rs::a_cover_is_uploaded_and_referenced` (delete), the
  multipart helper `multipart()` / `post_editor()` become a form-encoded
  helper; cover-proxy tests stay.

## Definition of done

- No page or form mentions a cover; the record we write has no
  `coverImage`; a foreign one survives an edit.
- `just check` and `just visual-check` pass.

## Decisions

Settled 2026-09-13: the editor switches to URL-encoded forms; `og:image`
keeps the generated placeholder until plan 07.
