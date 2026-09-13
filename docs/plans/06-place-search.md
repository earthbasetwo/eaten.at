# 06 · Place search with Open Places

Feedback 3: *Start the flow by having the user search for a place and
pick it from a list of results, then continue with the write-up.*
Feedback 5: *Store the place name and address that Overture returns.*
Feedback 6: *Use Open Places API for place search and lookup.*

Depends on plan 05. Two stages, each its own pull request: the client
(no UI), then the flow.

## What the API is (verified 2026-09-13)

Facts from `https://openplacesapi.com/openapi.yaml` (a copy is pinned
at `docs/plans/openplaces-openapi.yaml`) and the docs.

- One endpoint: `GET https://api.openplacesapi.com/v1/places`. Query:
  `q` (2–128 chars, ≥ 3 searchable letters), **`lat` and `lon`
  required**, `radius_mi` (≤ 50, default 25), `limit` (text search
  capped at 20, default 10), `mode` (`all`/`name`/`address`),
  `min_confidence`, `category`.
- **No lookup by id**, no bounding box, no autocomplete endpoint.
- Auth: `Authorization: Bearer opa_live_…`. Free tier 10k calls/month,
  120 requests/min per account; hard cap → 402 `quota_exhausted`,
  429 `rate_limited` with `Retry-After`. Error envelope
  `{error:{code,message,request_id}}`.
- Result: `place_id` (`"overture:08f2c…"`, whether the suffix is the
  raw GERS id is undocumented), `name`, `lat`, `lon`, `distance_mi`,
  `categories`, `category`, `address{formatted?,street?,locality?,
  region?,postal_code?,country_code?}` (all optional; `formatted` is
  absent in the only documented example), `phone?`, `website?`,
  `confidence`. `meta.data_release` names the Overture release.
- Terms: server-side only (keys must not be used in browsers; no CORS).
  Results may be cached and stored indefinitely. Overture attribution
  applies (CDLA-Permissive 2.0 / ODbL per record).

Two consequences drive the design. Every search needs a point, which the
browser supplies (settled: geolocation, below). And with no lookup by
id, everything we want to keep about a place must be captured at pick
time, which is what feedback 5 asks for anyway.

## Stage A · The client

`crates/eaten-at/src/places/` (`mod.rs`, `client.rs`):

- `PlacesClient` over `AppState::http()` (the guarded client: HTTPS
  only, public addresses only, timeouts, body cap). Config from
  `EATEN_AT_PLACES_API_URL` (default the production base) and
  `EATEN_AT_PLACES_API_KEY`. Without a key, search is disabled and the
  flow falls back to manual entry with a notice; the app
  logs once at startup.
- `search(q, point, radius) -> Result<Vec<Hit>, SearchError>`. `Hit`:
  `id`, `name`, `address: Option<String>` (`formatted`, else assembled
  from `street, locality, region, postal_code` joined with ", "),
  `lat`, `lon`, `distance_mi`, `category: Option<String>`,
  `website: Option<String>`. Serde structs with every non-required
  field `Option` and unknown fields ignored.
- `SearchError`: `Query(message)` (400: shown to the author),
  `Unavailable` (401/403 misconfiguration, 402, 429, 5xx, timeout: one
  calm sentence, details in the log with `X-Request-ID`), never the key.
- Cache: `Namespace::PlaceSearch`, TTL 1 hour, key
  `norm(q)|lat3|lon3|radius` (coordinates rounded to three decimals).
  Re-rendering a results page never re-spends quota.
- Id: strip the `overture:` prefix **only after** confirming with a real
  key that the suffix is the GERS id; otherwise store `place_id` as
  issued. Record the finding in `docs/decisions.md`. This is the one
  step that needs a key before code: sign up (free, no card), put the
  key in `.env` as `EATEN_AT_PLACES_API_KEY`, probe three known places.
- Tests: `wiremock` against a loopback URL under `Policy::for_tests()`:
  a good response, `formatted` missing, 400/402/429/500, timeout,
  cache hit makes no second request.

No geocoder. The point comes from the browser or from a stored visit.

## Stage B · The flow

Server-rendered, with one JavaScript island for the location. The
editor is the flow; there is no separate search page.

**The point.** The search form carries hidden `lat` and `lon` fields. A
small island (`static/locate.js`, within the shared 5 KB budget in
`assets.rs`) asks `navigator.geolocation.getCurrentPosition` when the
choosing state loads, fills the hidden fields, and shows "Searching
near you" in the mono voice; refused or unavailable, it shows "Location
unavailable" and the form still submits. The Permissions-Policy header
in `security.rs` becomes `geolocation=(self)`. The server falls back,
when the fields are blank, to the coordinates of the author's most
recent visit (plan 05's `latE6`/`lonE6`), and with neither it renders
the manual path only, with "Turn on location to search for the place."

This is the first page that needs JavaScript for a function rather
than a nicety, so D3 is amended: search needs location, which only the
browser can give; writing up a place by hand stays JavaScript-free.
Known limit: writing up a trip from home searches near home. The manual
path, or a later "search near" enhancement, covers it.

**New write-up.** `GET /write` with no place renders the *choosing*
state: heading "Where did you eat?", a `q` field ("Name of the place"),
the hidden point, one primary "Search", and beneath it the quiet link
"Not listed? Enter it by hand."

`GET /write?q=…&lat=…&lon=…` renders the same state with results beneath:
one card per hit (name in the display serif; address, distance, and
category in the mono voice; "Write about this place" as the card's
action). No results: "Nothing nearby called {q}." Search unavailable:
the notice and the manual path. Ten results, then "Not here? Try a more
exact name, or enter it by hand." Manual: `GET /write?manual=1` renders
the writing state with empty, editable name and address fields and no
id; this UI will be revisited in a later overhaul, so it stays plain.

Picking is a `GET` too: the card's action is a link
`/write?gers=…&name=…&address=…&lat=…&lon=…&website=…`, so a refresh,
the back button, and a bookmark all behave. That request renders the
*writing* state: the full editor with the Place group showing the name
and address as editable text fields, the id and coordinates as hidden
fields, and a "Change place" link button. If `website` is present the
first link row is prefilled with it as "Official site".

**Editing.** `GET /write/{rkey}` opens in the writing state. "Change
place" is a submit button, `action=change_place`, that re-renders the
same page in the choosing state with the rest of the form preserved as
hidden fields (the existing add/remove-row pattern); searching is then
`action=search` and picking is `action=pick:{index}` reading the hit
from the cached results. One results component serves both paths.

**Validation.** `gers_id` and coordinates present together or not at
all (manual entry has neither); name required; address optional; name
and address editable after the pick, since identity is the id (D33).

**Page head.** Choosing: kicker "Write", `h1` "Where did you eat?".
Writing: kicker "Write" or "Edit", `h1` the place's name. This restores
the heading plan 01 removed.

**Design.** `docs/design.md` gains the two editor states and a "Place
result" card (things a reader picks between are cards). Attribution: one
mono line under the results, "Places from Overture Maps", linked.
Principle 7 ("no script") gets the geolocation exception noted.

**Dev and visual check.** The local network has no internet. Extend
`scripts/dev-env.mjs` to start a stub HTTP server answering
`/v1/places` with canned JSON and write its URL into `.env.dev`, so
`just visual-check` can render the choosing state, results (with the
point passed in the query, since headless Chrome grants no location),
the writing state after a pick, the manual state, and the unavailable
state. The visual check also asserts the island logs no error when
geolocation is denied.

## Touchpoints

- New: `places/`. Config: `settings.rs`, `state.rs` (`AppConfig`),
  README's variables table and routes table.
- `editor/form.rs` (place fields, `Action::Search/Pick/ChangePlace`),
  `editor/draft.rs`, `editor/view.rs` (two states, results component),
  `routes/write.rs` (query-string handling on `GET /write`).
- `cache.rs`: `Namespace::PlaceSearch`.
- `security.rs`: `geolocation=(self)`; `eaten-at-web/src/assets.rs`:
  the new island in `INLINE_SCRIPTS` and the budget; `layout.rs`
  `scripts` on the choosing state.
- `docs/decisions.md`: D34 Open Places; D35 location from the browser
  (D3 amended); D36 the `place_id` finding.
- Tests: `routes.rs` for each state and action, `insta` snapshot of the
  results card, client tests as above.

## Definition of done

- With a key, a new write-up starts at a search; the form is reached
  by picking a result or by choosing to enter the place by hand.
- A pick stores `gersId`, name, address (and coordinates); an edit can
  change the place without losing the prose.
- Quota and rate-limit responses degrade to a notice, never a 500.
- `just check` and `just visual-check` (with the stub) pass.

## Decisions

Settled 2026-09-13:

1. **The point comes from browser geolocation**, with the author's last
   visit as the server-side fallback. No geocoder, no "Near" field.
2. **Manual entry is supported**: a place can be written up without an
   Overture match. The search UI is deliberately plain; a later
   overhaul will revisit it.
3. **Prefill the official-site link** from `website`.
4. **Name and address stay editable** after a pick.
5. **Radius**: the API default, 25 miles, not exposed.
