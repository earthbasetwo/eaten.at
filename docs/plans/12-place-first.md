# 12 · A new visit starts with the place

Feedback 19: *The new-visit UI should begin with selecting a place. Use
Open Places API for autocomplete of place names, with a reverse-geo
lookup of the IP for the location sent to the API.*
Feedback 20: *Selecting from the API's list is not required: the user
may enter a name (required) and an address (optional) manually.*
Feedback 21: *After selecting a place, the user should be taken to the
main write/edit UI with that place's info pre-filled, but still
editable.*

Large. After plan 10 (the combobox) and plan 08 (no publication fields
in the editor). Two stages, each its own pull request: the location
(server side, no UI change), then the choosing page.

## What plan 06 already did, and what changes

Plan 06 made the editor open in a *choosing* state: a search box, a
point from the browser's geolocation, results as cards, a "Not listed?
Enter it by hand" link, then the *writing* state with the place fields
prefilled and editable. Feedback 21 is therefore already true, and the
Open Places facts in plan 06 still hold (re-checked 2026-09-13 against
the live `openapi.yaml`: one endpoint, `lat` and `lon` required, no
autocomplete and no lookup by id, `limit` 1–50, `offset` up to 950,
monthly quota headers, 429 on burst).

What changes:

1. **The point comes from the request's IP**, not from a geolocation
   prompt. The location island goes (D35 amended).
2. **The search box suggests as you type.** The same search, through
   our server, rendered by plan 10's combobox.
3. **Manual entry is on the choosing page itself**: name and address
   fields under the search, not a link that flips the form.

## Stage A · Locating by IP

`crates/eaten-at/src/geoip.rs`: `IpLocator` with one method,
`locate(ip) -> Option<Point>`, and the request-side helper
`client_ip(headers, peer) -> Option<IpAddr>` that takes the first
address in `X-Forwarded-For` when the app is behind its proxy (it
already trusts `X-Forwarded-Proto`), else the peer address. Loopback,
link-local, and private ranges locate to nothing.

Settled 2026-09-13: **a local IP database, DB-IP's IP-to-City Lite.**
The alternatives considered were proxy-supplied location headers
(Cloudflare's managed transform: free, but only if the deployment sits
behind Cloudflare, which is not known), a hosted lookup API (ipinfo.io
and the like: a fourth upstream in the path of every first keystroke,
a key, a quota, and every author's IP sent to a third party), and
MaxMind's GeoLite2-City (the same file format and reader, better
regarded on the accuracy tail, but an account, a licence key, and an
EULA with conditions to keep reading). DB-IP Lite is CC BY 4.0, needs
no account, and is a plain monthly download; if its accuracy turns out
to matter, GeoLite2 is a file swap with no code change.

Concretely:

- `IpLocator` is `Database(maxminddb::Reader)` loaded from
  `EATEN_AT_GEOIP_DB`, or `None` when the variable is unset (search is
  then offered only near the last visit). The setting is a file path and
  nothing else; the code assumes the GeoIP2 City layout, which DB-IP
  Lite documents as compatible, and names no vendor. The `maxminddb`
  crate is pure Rust; read the file into memory rather than `mmap`, so
  a refresh can replace it on disk without affecting the running
  process, which picks it up on restart.
- Rows without coordinates, and addresses the database does not know,
  locate to nothing. The city name (`city.names.en`), when present,
  feeds the status line.
- `just geoip-refresh`: one `curl` of the current month's
  `dbip-city-lite-YYYY-MM.mmdb.gz` into the path `EATEN_AT_GEOIP_DB`
  names, decompressed, with a `.tmp` and rename so a half-download is
  never read. Documented in the README beside the variable, with a
  note to run it monthly (a cron in production).
- Attribution: CC BY asks for credit, so the choosing page's mono line
  becomes "Places from Overture Maps · Location by DB-IP", each linked.
- Tests use the small `.mmdb` fixtures MaxMind publishes in its
  MaxMind-DB repository (same format), checked in under
  `tests/fixtures/`, so no test depends on the production file.

Development: `EATEN_AT_DEV_LOCATION=40.6888,-73.9799` in `.env.dev`
answers every request with that point, so the stub network and the
visual check have somewhere to search. It is a `Dev` setting like the
others and refused outside the local network.

The fallback chain on the choosing page, in order: the IP's point; the
author's most recent visit with coordinates (plan 06's fallback, kept);
nothing, in which case the search is hidden and only manual entry is
offered, with "Location unknown, so search is off; enter the place
below." The status line under the search box says which: "Searching
near Brooklyn" (when the database names the city) or "Searching near
your last visit."

Known limits, stated plainly. IP location is city-level at best, and on
a phone it is weaker: carrier networks pool traffic, and Safari's
iCloud Private Relay egresses from Apple's addresses, which map to a
coarse region by design. A phone in Brooklyn may search near Newark.
The 25-mile radius absorbs most of that; the manual path and the
last-visit fallback cover the rest. And, unchanged from plan 06,
writing up a trip from home searches near home. Browser geolocation is
not kept as a refinement (settled below); if the phone case proves
bad in practice, a "Use my precise location" link button that reuses
plan 06's island is the first thing to try.

Stage A ships behind the existing form: `locate()` in `routes/write.rs`
consults the IP before the last visit, `near_lat`/`near_lon` stop being
form fields, and the location island and its `data-locate` hook are
removed with `geolocation=(self)` going back to `geolocation=()`. The
visual check's `point` fill is replaced by `EATEN_AT_DEV_LOCATION`.

## Stage B · The choosing page

`GET /write` (and `action=change_place` on an edit) renders:

1. **Page head.** Kicker "Write" (or "Edit"), `h1` "Where did you eat?"
2. **The search.** Label "Name of the place", one text input with
   `data-suggest="/write/suggest"` and placeholder "Start typing…", the
   status line under it in the mono voice. With JavaScript, the combobox
   from plan 10 shows up to six matches as you type: the name in the
   sans, the address and distance in the mono voice. Picking one submits
   the form as `action=pick:N` with the query the suggestions came from
   (the option carries `data-q` and `data-i`; the island copies `data-q`
   into the field before submitting, so the server's pick re-reads the
   same cached search). Without JavaScript, the "Search" button and the
   result cards from plan 06 do the same job; they stay, the button
   hidden with JavaScript on.
3. **Or enter it yourself.** A hairline, a kicker "Or enter it
   yourself", then Name (required) and Address (optional) as visible
   fields, and a secondary pill "Continue with this place" that submits
   `action=manual`. The server takes the name and address from these
   fields into the writing state with no listing behind them. A blank
   name is the one error this page can show.
4. **Attribution**, one mono line: "Places from Overture Maps ·
   Location by DB-IP", each linked (Stage A).

Then the writing state, as built by plan 06: the Place group shows the
name and address as editable fields, the mono line saying where the
place came from, and "Change place". Nothing in the writing state
changes in this plan.

### The suggest endpoint

`GET /write/suggest?q=…`, signed-in only (`RequireUser`), JSON:

```json
{ "q": "katz", "near": "Brooklyn", "hits": [
  { "i": 0, "name": "Katz's Delicatessen", "detail": "205 E Houston St, New York, NY 10002 · 2.1 mi" }
] }
```

- It locates the request exactly as the page does and calls
  `search_places(q, point)`, so the result is the same cached search
  the pick re-reads. `i` is the index into that search.
- Rules that keep the quota sane: the island sends nothing under three
  characters and debounces at 300 ms; the server refuses more than 30
  requests a minute per session (a token bucket in memory keyed by the
  session token, `429` with a JSON error, the island closes the listbox
  and stays quiet); the one-hour search cache in `PlaceSearch` means a
  repeated prefix costs nothing. Rough arithmetic: a place typically
  takes three to five suggest calls, so the free tier's ten thousand
  calls a month is two to three thousand write-ups; enough for v1, and
  the app logs a warning when `x-quota-remaining` drops under ten
  percent so the paid tier can be turned on in time.
- Errors: `Query` from the API becomes an empty list (a typed prefix is
  often too short for the API's own rule of three letters);
  `Unavailable` becomes `{ "hits": [], "error": "unavailable" }` and
  the island shows one calm line in the listbox's place, "Search isn't
  answering; enter the place below."
- No point: `{ "hits": [], "error": "no_location" }`; the island shows
  nothing, since the page already said search is off.
- The response is `Cache-Control: private, no-store`; the server-side
  cache is the one that matters.

### The island

`static/place-suggest.js` (~1.2 KB) binds `input[data-suggest]` to the
combobox with a source that fetches `{data-suggest}?q=…` same-origin,
renders `name` and `detail`, and on pick sets the input to the option's
`data-q`, sets the form's hidden `action` value to `pick:{i}`, and
submits. `connect-src` needs nothing: same origin. It ships with the
combobox on the choosing state only.

## Design

- `docs/design.md`: the choosing recipe is rewritten (search combobox
  with status line, hairline, manual fields with a secondary pill,
  attribution); principle 7 loses the geolocation exception and gains
  the suggest fetch (same origin); the **Place result** card stays for
  the no-JS path. D35 is amended in `docs/decisions.md`: the point
  comes from the request's IP (D44), browser geolocation is no longer
  asked for.
- `app.css`: the manual-entry group (`.manual-entry` as a fieldset with
  the hairline above), the hidden-with-JS search button
  (`html.js .search-field button` via a one-line class set by the
  combobox script), and the combobox voices from plan 10 apply as is.

## Touchpoints

- New `geoip.rs`; `settings.rs` and `state.rs` (`GeoIpConfig`,
  `EATEN_AT_GEOIP_DB`, `EATEN_AT_DEV_LOCATION`); README's variables
  table and the `geoip-refresh` recipe in the `justfile`;
  `docs/local-dev.md`. `Cargo.toml`: `maxminddb`. Fixtures under
  `crates/eaten-at/tests/fixtures/`.
- `routes/write.rs`: `locate()` order, `suggest` handler, the manual
  action reading the visible fields, `near_lat`/`near_lon` removed;
  `editor/form.rs` (fields), `editor/view.rs` (the choosing page).
- `app.rs`: `GET /write/suggest` under the editor's router. A small
  in-memory rate limiter in `security.rs` or its own `ratelimit.rs`.
- `places.rs`: log the quota header; `Hit` gains nothing (`detail` is
  composed in the route).
- Remove `static/locate.js`, `LOCATE_SCRIPT`, the `data-locate` hook;
  `security.rs` Permissions-Policy back to `geolocation=()`. Add
  `static/place-suggest.js` and `PLACE_SUGGEST_SCRIPT` within plan 10's
  budget.
- `scripts/dev-env.mjs`: `EATEN_AT_DEV_LOCATION` in `.env.dev`; the
  Open Places stub already answers by query. `scripts/visual-check.mjs`:
  `write` expects the combobox input; `write-suggest` types `noo` and
  expects `[role="option"]`; `write-pick` picks through the listbox;
  `write-manual` fills the name and continues; `write-manual-blank`
  expects the one error; `write-no-location` runs with the dev location
  unset and expects the search hidden and manual entry shown; the no-JS
  path is covered by `write-search` as today.
- `docs/decisions.md`: D44 IP location, D45 suggestions through our
  server with the quota rules.
- Tests: `geoip.rs` (forwarded-for parsing, private ranges locate to
  nothing, a known address in the fixture locates to its coordinates
  and city, an unknown one to nothing, a missing file is a startup
  error); `routes.rs` (suggest
  returns the same indices the pick reads; 429 after the bucket empties;
  signed out is a redirect; manual with a blank name is 422; the
  choosing page has no `near_lat`); `wiremock` for the unavailable
  shape.

## Definition of done

- A new write-up opens on "Where did you eat?", suggests places as the
  author types near where they are, and a pick lands in the editor with
  the name, address, id, and coordinates filled and editable.
- The same page takes a name and an optional address by hand.
- With JavaScript off, the search button and result cards still work;
  with no location, manual entry alone is offered and nothing errors.
- No geolocation prompt anywhere; no key in the browser.
- `just check` and `just visual-check` pass.

## Decisions

Settled 2026-09-13:

1. **IP location source: DB-IP IP-to-City Lite**, as a local `.mmdb`
   file, configured by path only. Reasons and the alternatives are in
   Stage A.
2. **Browser geolocation is dropped**, not kept as an opt-in. The
   feedback asks for IP location, city accuracy is enough for a
   25-mile search, and a prompt is a step. D35 is superseded.

3. **Six suggestions in the listbox, ten on the no-JS results page**
   (both from one search with `limit=10`).
4. **Radius: the API default, 25 miles.** A mobile carrier's IP can be
   further off than that; the manual path is the answer, not a bigger
   radius that floods the list.
5. **Rate limits: three characters, 300 ms, 30 requests a minute per
   session.** All three are constants in one place and can move without
   a design change.
