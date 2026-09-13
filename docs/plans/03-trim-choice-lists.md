# 03 · Trim the choice lists

Feedback 7: *Remove the "Other" meal option.*
Feedback 8: *Limit link types to "Official site" and "Other".*

Two small changes sharing one mechanism. They can ship as one pull
request or two; the shared refactor (part 0) goes first either way.

## Part 0 · `Choice` without a free-text "Other"

`editor/form.rs` models every `knownValues` select as
`Choice<K> { Unset, None, Known(K), Other }` with a sibling free-text
field (`meal_other`, `link_service_other_N`) that "Other" reveals. That
free-text field exists for one reason: a value another client wrote must
survive a round trip through our editor (`docs/lexicons.md`: values
outside `knownValues` **MUST** be preserved on rewrite).

Replace it with `Choice<K> { Unset, None, Known(K), Foreign(String) }`:

- `from_record(Some(v))` with `v` unknown gives `Foreign(v)`.
- The select renders a `Foreign` value as one extra `<option>` whose
  value and label are the string as written, selected. There is no text
  input.
- `from_value` on submit: known string → `Known`; `""` → `Unset`;
  `"none"` → `None`; anything else → `Foreign(s)`, still bounded by the
  lexicon's 640-byte cap in `validate`. (A hand-edited form can post any
  string; that is no worse than today's free-text field and the lexicon
  allows it.)
- `SERVICE_OTHER`, `KnownSelect.other_name`, `Blank`, and the
  `*_other` form fields are deleted.

## Part A · Meal

- `meal_select`: options are "Not said" and the five known meals, plus a
  `Foreign` option only while editing a document that has one.
- `EditorForm.meal_other` and the `meal_other` field go; the
  `MAX_SERVICE_BYTES` check moves to the `Foreign` value.
- Lexicon unchanged (`knownValues` is already a suggestion, not an enum).

## Part B · Link types

- `lexicons/at.eaten.place.json`, `#externalUrl.service.knownValues`:
  `["officialSite"]`. Drop `menu` and `reservations` from the
  description. `KnownService` keeps only `OfficialSite`.
- The select per link row: "Official site" and "Other" (`None`: no
  `service` written; the link is labelled by its label or its host, as
  today). Default is "Other". A `Foreign` value renders as above.
- `link_service_other_N` fields go.

## Touchpoints

- `editor/form.rs`, `editor/draft.rs`, `editor/view.rs`.
- `eaten-at-atproto/src/lexicon/at_eaten.rs`: `KnownService`.
- `lexicons/at.eaten.place.json`; `docs/lexicons.md` (`#externalUrl`);
  `scripts/dev-env.mjs` seeds a `menu` link, which becomes `Foreign`
  (keep it: it proves preservation on the edit page).
- Tests: `form.rs::choice_values`, `draft.rs::good_form` (a `shop` link
  becomes `Foreign("shop")`), `a_foreign_document_round_trips_unchanged`
  (still must pass byte-for-byte), `routes.rs::good_fields` and
  `editing_prefills_from_the_document_and_keeps_foreign_values`.
- `docs/decisions.md`: D30 "One link type, `officialSite`; everything
  else is a plain link." D31 "No free-text vocabulary in the editor;
  foreign values are preserved but not authored."
- `just lexicons-check` will report drift until republished.

## Definition of done

- Neither select offers a text field; a foreign value on an existing
  document survives an edit unchanged.
- `just check` and `just visual-check` pass.

## Decisions

1. **Label for a foreign option.** The string as written (e.g. `tea`),
   not annotated. Recommended; anything else invents vocabulary.
2. **One PR or two.** Recommended: one, since part 0 is the bulk.
