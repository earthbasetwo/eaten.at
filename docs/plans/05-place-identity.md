# 05 · Place identity: one GERS id

Feedback 4: *Remove the concept of multiple external IDs. For now, use
only Overture Places, identified by GERS ID.*
Feedback 5 (the model half): *Store the place name and address that
Overture returns directly in the app's own model.*

Plan 06 fills these fields from a search. This plan changes the record
and the code that reads and writes it, and leaves the editor with a
plain text field for the id until the search replaces it.

## Record

`lexicons/at.eaten.place.json`:

- Remove `ids` and the `#externalId` def.
- Add `gersId`: string, `maxLength` 128. "The place's Overture Maps GERS
  id, exactly as Overture issues it. Two places with the same `gersId`
  are the same place. Names and addresses are for people."
- `name` and `address` keep their constraints; their descriptions say
  they are "as the author's client found them, normally from Overture".
- `latE6` and `lonE6`: integers, the place's coordinates in
  microdegrees (`40688838` for 40.688838), as Overture gives them.
  Atproto lexicons have no float type; microdegrees are exact and
  sortable. Optional in the schema.

`gersId` is optional in the schema, so a foreign place without one still
reads. Our editor leaves it absent only for a place entered by hand
(plan 06).

Supersedes D11. New decision row: "D33 Place identity is the Overture
GERS id in `at.eaten.place.gersId`. No other id services."

## Code

`eaten-at-atproto/src/lexicon/at_eaten.rs`:

- `Place { gers_id: Option<String>, lat_e6: Option<i32>, lon_e6: Option<i32> }`
  with range checks on read (out of range reads as absent); delete
  `ExternalId`, `KnownIdService`, `url_for`.
- `Place::same_as`: both `gers_id` present and equal.

`eaten-at/src/view.rs`: `place_links` replaces the Google and Apple
map links with one "Map" link to OpenStreetMap at the exact point
(`https://www.openstreetmap.org/?mlat=…&mlon=…#map=18/…/…`) when the
place has coordinates.

`eaten-at/src/editor/`:

- `IdField`, `RowKind::Id`, `MAX_IDS`, the id rows and their actions,
  `MAX_ID_BYTES`/`MAX_SERVICE_BYTES` uses for ids: deleted.
- One field, "Overture GERS id (optional)", `gers_id`, in the Place
  group, with the hint "Plan: this becomes a search." Validation: trim,
  ≤ 128 bytes, no whitespace inside. We do not own the format, so no
  stricter check.
- `preserve_unknown_fields` no longer matches ids.

`scripts/dev-env.mjs`: seed `gersId` on each place (the third seed
already uses an Overture-shaped id).

## Touchpoints

- `lexicons/at.eaten.place.json`, `docs/lexicons.md` (`at.eaten.place`
  section, the identity paragraph, the `#externalId` section removed),
  `docs/decisions.md` (D11 marked superseded; D33).
- `at_eaten.rs` tests: `visit_round_trips…`, `known_values_are_exact_matches`,
  `id_services_link_where_they_can` (delete), `places_match_on_a_shared_id`.
- `editor/*` tests, `routes.rs::good_fields` (`id_service_0`/`id_value_0`
  → `gers_id`), `view.rs::place_links_label_urls_and_add_map_pages_for_known_ids`,
  the publish snapshot.
- `docs/design.md`: the editor's "Repeated fields (ids, links)" line.
- `just lexicons-check` reports drift until republished.

## Definition of done

- A visit round-trips with `place.gersId`; `ids` is gone from the
  written record; a foreign `ids` array on an edited document is carried
  over untouched as an unknown field.
- `just check`, `just visual-check` pass.

## Decisions

Settled 2026-09-13:

1. **Map link:** store coordinates and link OpenStreetMap at the exact
   point. Plan 06 also uses the stored coordinates as the default search
   point when the browser gives none.
2. **Coordinates as lexicon fields:** integer microdegrees, `latE6` and
   `lonE6`.
