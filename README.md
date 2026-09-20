# eaten.at

Write-ups on the AT Protocol, rendered one publication at a time.

Authors write `site.standard.document` records into their own repositories
whose `content` is an `at.eaten.visit`: a place, a date, an optional
rating, and the prose. This app renders those documents: it is a reader
over other people's repositories, not a warehouse. There is no global feed
and no content index.

This repository was forked from album-report on 2026-09-12 and stripped to
the parts that are not about albums. The decisions it inherits, and the
ones eaten.at has made since, are listed in `docs/decisions.md`; the
schemas are described in `docs/lexicons.md`.

## Running

Requires Rust 1.89 or newer. SQLite is bundled.

```
cargo run -p eaten-at
```

Then open `http://127.0.0.1:3000/`. Type a handle into the lookup form, or go
straight to a repository by DID: `/at/did:plc:…/`.

`just check` runs formatting, clippy with warnings denied, and the tests.
`just run` runs the server. `just visual-check` renders every page, signed
out and signed in, in headless Chrome against the local network and fails on
errors, CSP violations, missing fonts, or horizontal scrolling (see
`docs/local-dev.md`).

For editing, `just watch` automatically rebuilds and restarts when source files
or embedded assets change (requires `watchexec`). Refresh the browser once the
build finishes. See `docs/local-dev.md` for the local-network variant, `just watch-dev`.

### Configuration

Everything is an environment variable; a `.env` file is loaded by `just`.

| Variable | Default | Meaning |
|---|---|---|
| `EATEN_AT_LISTEN` | `127.0.0.1:3000` | Socket address to bind. |
| `EATEN_AT_DB` | `eaten-at.db` | SQLite file for the cache, OAuth state, sessions, and hosting claims. |
| `EATEN_AT_PUBLIC_URL` | `http://<listen address>` | Our public origin, used for absolute URLs in meta tags and feeds. Set to `https://eaten.at` in production. |
| `EATEN_AT_OAUTH_KEY_FILE` | unset | Path of the private JWK that makes the app a confidential OAuth client; created on first start. Unset, the app is a public client. |
| `EATEN_AT_BSKY_APPVIEW` | `https://public.api.bsky.app` | The Bluesky AppView that comment threads are read from (unauthenticated `getPostThread`, cached five minutes) and that the sign-in and landing pages ask for handle suggestions from the browser (`searchActorsTypeahead`; their CSP allows this one origin). |
| `EATEN_AT_PLACES_API_URL` | `https://api.openplacesapi.com` | The Open Places API, which serves Overture Maps places for the editor's place search. |
| `EATEN_AT_PLACES_API_KEY` | unset | The Open Places API key. Unset, place search is disabled and the editor takes places by hand. Never used from a browser. |
| `EATEN_AT_GEOIP_DB` | unset | Path of an IP-to-city database in the MaxMind DB format with the GeoIP2 City layout: DB-IP's IP-to-City Lite (CC BY 4.0), fetched by `just geoip-refresh`. The editor's place search looks near where the request's IP is. Unset, it looks near the author's last visit only. Read into memory at startup; refresh monthly and restart. |
| `RUST_LOG` | `info` | Log filter. `debug` shows cache misses, skipped records, and upstream fallbacks. |

The app expects to sit behind a TLS-terminating reverse proxy that sets
`X-Forwarded-Proto`. It never listens for TLS itself.

For running against a local atproto stack instead of the public network,
including the `EATEN_AT_DEV_*` variables, see `docs/local-dev.md`.

### Publishing the lexicons

```
EATEN_AT_LEXICON_IDENTIFIER=eaten.at just lexicons-check     # diff, exit 1 on drift
EATEN_AT_LEXICON_APP_PASSWORD=… just lexicons-publish        # write what differs, then verify via DNS
```

## Routes

| Route | What it is |
|---|---|
| `/` | Landing page with the handle lookup form. |
| `/@{handle}` | Resolves the handle now and redirects to the DID form. Never rendered. |
| `/at/{did}/` | The account's eaten.at publication (the one its preferences name); for a repository without one, a lone publication or a chooser. |
| `/at/{did}/{pub}/` | A publication's write-ups, newest first. |
| `/at/{did}/{pub}/{doc}` | One document, with a canonical link to its publication origin. |
| `/at/{did}/{pub}/tagged/{tag}` | Write-ups in that publication carrying a tag. |
| `/at/{did}/{pub}/feed.xml` | RSS. |
| `/img/{did}/{doc}` | The document-image proxy: the first photo, else a `coverImage` another client set, else a generated placeholder. `?size=og` gives a 1200×630 rendition; `?kind=icon` a publication icon. |
| `/img/{did}/{doc}/{cid}` | One of the document's photos, `?size=thumb` (a 400px square), `?size=card` (up to 960px wide, cropped to 3:2, for listing cards), or `?size=full`. Only CIDs the document lists. |
| `/write`, `/write/{doc}` | The editor, for signed-in authors, set as prose. A new write-up starts by choosing the place: its name, with suggestions as you type near where the request is from, and its address, both by hand if need be. Editing manages the write-up's photos in place. |
| `/write/suggest` | Place suggestions for the editor's place field (JSON, signed-in only, rate-limited). |
| `/write/upload` | The editor's photo upload (signed-in only): each file is re-encoded and uploaded to the author's repository at once, and answered as the blob reference the form carries until the record is written. |
| `/write/photo/{cid}` | One of the signed-in author's own blobs, at `?size=thumb` or `full`, for the editor's tiles before a record lists it. |
| `/write/{doc}/photos` | Add, caption, reorder, and remove a write-up's photos without JavaScript; every action writes the record at once (JSON when asked). |
| `/settings` | The author's one publication: its name, description, and address (a hosted subdomain or their own domain). Creates it on the first save if a publish has not already. |
| `/healthz` | Liveness. |

## Layout

- `crates/eaten-at` — the application: routes, cache, read path, editor, publishing.
- `crates/eaten-at-atproto` — identity resolution, repository I/O, OAuth, the guarded HTTP client, lexicon types.
- `crates/eaten-at-web` — HTML components, the stylesheet, markdown rendering, theming.
- `lexicons/` — our schemas (`at.eaten.*`) and pinned copies of the upstream `site.standard.*` schemas.

## Tests

```
cargo test --workspace
cargo test --workspace -- --ignored   # live checks against PLC, DNS, and the Bluesky AppView
```

Network-touching tests use a mock server on loopback; the guarded HTTP client
has a test policy that permits it. Rendered HTML is pinned with `insta`
snapshots under `tests/snapshots/`.
