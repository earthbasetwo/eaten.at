# 10 · Handle autocomplete on sign-in

Feedback 17: *The sign-in flow should autocomplete the AT Protocol
handle with `app.bsky.actor.searchActorsTypeahead` on
`https://public.api.bsky.app/xrpc/…?limit=5&q=…`: no auth,
`credentials: 'omit'`.*
Feedback 18: *The UI should indicate "start typing" or similar, so the
user knows they won't have to enter their entire handle by hand.*

Medium. After plan 09 (the landing's lookup field gets the same
island). Builds the shared combobox that plan 12 reuses for places.

## What the endpoint is (verified 2026-09-13)

`GET https://public.api.bsky.app/xrpc/app.bsky.actor.searchActorsTypeahead`

- Params: `q` (a prefix, "not a full query string"), `limit` (1–100,
  default 10). `term` is deprecated. No auth.
- Response: `{ actors: [profileViewBasic] }`, each with `did`, `handle`,
  `displayName?`, `avatar?` (a `cdn.bsky.app` URL), `labels`, and more.
- CORS: `access-control-allow-origin: *`, so a browser on eaten.at can
  call it directly with `credentials: 'omit'`. No preflight for a plain
  GET.
- It answers from the Bluesky AppView's index. An account whose PDS is
  not indexed by Bluesky (a self-hosted PDS that never federated) will
  not appear; its handle still works typed in full and resolved by the
  server, as today. The UI says so.

## The island

This is the second function-not-nicety script after the location island
(D35), and unlike that one it makes a request to a third party from the
browser. Principle 7 in `docs/design.md` ("no page makes a third-party
request") is amended: the sign-in and landing pages call the Bluesky
AppView for handle suggestions, and nothing else does.

Two files, both inline with the nonce, both counted against the budget:

- `static/combobox.js` (~1.6 KB): a generic, dependency-free combobox
  over an `<input>` and an injected listbox, following the WAI-ARIA
  pattern: `role=combobox`, `aria-expanded`, `aria-controls`,
  `aria-activedescendant`; options as `role=option`; Down/Up move, Enter
  picks, Escape closes, clicking picks; the listbox is removed when the
  input loses focus. It takes a `source(q, signal) -> Promise<items>`
  and a `render(item) -> {label, detail}` and a `pick(item)`; debounces
  (350 ms here), aborts the in-flight request when the query changes
  (`AbortController`), and does nothing under `minChars` (3 here). Plan 12 reuses
  it unchanged for places.
- `static/handle-typeahead.js` (~0.8 KB): binds every
  `input[data-typeahead]` to the combobox with a source that fetches
  `{data-typeahead}/xrpc/app.bsky.actor.searchActorsTypeahead?limit=5&q=…`
  with `credentials: 'omit'`; renders `displayName` as the label (or the
  handle when there is none) and `@handle` as the detail; picking writes
  the handle into the input. Fetch failures close the listbox silently;
  the form is untouched.

`data-typeahead` carries the AppView origin the server is configured
with (`EATEN_AT_BSKY_APPVIEW`), so the island and the CSP always agree
and the local network can point it at a stub.

**Budget.** `JS_BUDGET_BYTES` is 5 KB and the two existing islands use
4.7 KB. This plan adds about 2.4 KB and plan 12 about 1.5 KB more while
removing the 1 KB location island. Raise the budget to 10 KB, and add a
second assertion: no single page ships more than 7 KB (each route lists
its scripts, so the test can sum per page). The point of the budget was
never the number; it is that every page works without any of it, which
each plan's no-JS test keeps proving.

**Content Security Policy.** `connect-src` is not set today, so
`default-src 'self'` covers it and the fetch would be blocked. Add a
per-page allowance: `Nonce::csp()` stays the strict default, and
`Nonce::csp_connecting(&[origin])` yields the same policy with
`connect-src 'self' {origin}`. The sign-in and landing handlers set the
header themselves (the middleware leaves a header a handler set alone,
as it does for the image proxy). No other page's policy changes.

**No avatars.** Showing the avatar would mean `img-src` allowing
`cdn.bsky.app` and a request per suggestion. Display name and handle
disambiguate well enough for v1; see the decisions.

## The pages

**Sign in.** The field's label becomes "Your handle"; placeholder
"Start typing your handle…"; the hint under it, in the sans at small
size: "Suggestions from Bluesky appear as you type. Any AT Protocol
handle works typed in full, and so does a DID." The hint is served in
the HTML, so it is true without JavaScript too (the second sentence).
The listbox appears under the field, on the raised surface, in the
field's width, one option per line: the name in the sans, the handle
in the mono voice.

**Landing (signed out).** The lookup field from plan 09 gets
`data-typeahead` too and its placeholder becomes "Start typing a
handle…". The submit still goes to `/lookup`.

**Lookup error page.** Same form, same island.

## Local development and the visual check

The local network has no AppView (`docs/local-dev.md`). Extend the stub
server in `scripts/dev-env.mjs` (it already stands in for Open Places)
with `/xrpc/app.bsky.actor.searchActorsTypeahead` answering three canned
actors for any query, CORS `*`, and write `EATEN_AT_BSKY_APPVIEW` into
`.env.dev` pointing at it. Comment threads then read from the stub too,
which returns `notFoundPost` for any thread; `docs/local-dev.md` notes
the trade (today it says the public AppView is used for threads).

`scripts/visual-check.mjs` gains `login-typeahead`: type `al` into the
field and expect `[role="listbox"] [role="option"]`; and asserts the
page logs no CSP violation (the runner already fails on those).

## Touchpoints

- New `static/combobox.js`, `static/handle-typeahead.js`;
  `eaten-at-web/src/assets.rs` (`COMBOBOX_SCRIPT`,
  `HANDLE_TYPEAHEAD_SCRIPT`, `INLINE_SCRIPTS`, the budget and the
  per-page assertion).
- `security.rs`: `csp_connecting`; a test that the default policy is
  unchanged and the connecting one adds exactly one origin.
- `routes/auth.rs` (login page: hint, `data-typeahead`, scripts, CSP),
  `routes/landing.rs` and `routes/lookup.rs` (same for the lookup form).
- `eaten-at-web/src/components.rs`: `LookupForm` gains `typeahead:
  Option<&str>` (the AppView origin) and the hint.
- `app.css`: `.combobox-list`, `.combobox-option`,
  `.combobox-option[aria-selected="true"]`, the option's name and
  detail voices. Under `components`.
- `docs/design.md`: principle 7 amended; a **Combobox** component entry;
  the sign-in and landing recipes.
- `docs/decisions.md`: D42 handle suggestions from the Bluesky AppView
  in the browser (the one third-party request a page makes, and only
  those pages); D43 the JavaScript budget restated as 10 KB total, 7 KB
  per page.
- `scripts/dev-env.mjs`, `scripts/visual-check.mjs`, `docs/local-dev.md`
  as above. README: `EATEN_AT_BSKY_APPVIEW` now also feeds the
  typeahead.
- Tests: `routes.rs` (login page carries `data-typeahead` with the
  configured origin and the `connect-src` header names it; the landing
  does too; the document page's policy has no `connect-src`).

## Definition of done

- On `/login` and `/`, typing three characters shows up to five Bluesky
  accounts; arrow keys and Enter fill the field; the form submits as
  before with or without a pick.
- With JavaScript off, both forms work exactly as today.
- The CSP on every other page is unchanged.
- `just check` and `just visual-check` pass.

## As built (2026-09-13)

As planned, with these notes.

- The combobox came out at 3.5 KB, not 1.6 KB: the ARIA bookkeeping
  and the abort-on-newer-query logic are most of it. The page set is
  4.7 KB against the 7 KB per-page cap and everything ships at 9.4 KB
  against the 10 KB total; plan 12 replaces the 1 KB location island
  with a suggestion adapter of about the same size.
- The connecting policy is set by the handler through
  `security::allow_connect`, on the landing page (signed out), the
  sign-in page in every state, and the lookup-error page.
- The listbox is inserted after the `.lookup-row`, not after the input,
  so it sits under the row rather than inside the flex row.
- The visual check gained a `type` step (an `input` event, then a wait
  for a selector) for pages whose change is not a navigation; it checks
  suggestions on both the sign-in and the landing page.
- The stub in `dev-env.mjs` now also answers the typeahead, and
  `.env.dev` points `EATEN_AT_BSKY_APPVIEW` at it, so comment threads
  read as not found on the local network (noted in `docs/local-dev.md`).

## Decisions

Settled 2026-09-13:

1. **No avatars in the suggestions.** Display name and handle only.
   Avatars would have meant `img-src` allowing `https://cdn.bsky.app`
   on those two pages and up to five image requests per settled
   keystroke.

2. **Minimum characters: 3.** The endpoint accepts 1; three keeps a
   single letter from fanning out to Bluesky.
3. **Debounce: 350 ms**, cancelling the previous request.
4. **The hint goes under the field, served**, so it is always true,
   rather than placeholder text alone, which disappears on the first
   keystroke.
