# atrium-oauth evaluation

Decision input for C3.1 (OAuth client). Evaluated `atrium-oauth` 0.1.7 from
its published source on 2026-09-07, per plan §6.2 and §9 (Phase 1).

## What the plan needs

| Requirement | Plan reference | atrium-oauth 0.1.7 |
|---|---|---|
| Pushed Authorization Requests (PAR) | §5.4 | Yes. `oauth_client.rs` uses `pushed_authorization_request_endpoint` when the server advertises it and errors when the server requires PAR but has none. |
| DPoP key management and proofs | §5.4 | Yes. `http_client/dpop.rs` wraps any HTTP client with DPoP proofing; keys are ES256 via its `jose` module. |
| Token refresh | §5.4 | Yes. `server_agent.rs` exposes `refresh`, and `oauth_session` refreshes on demand. |
| Granular `repo:` scopes | §4.4 | Passable. `Scope` is `Known(KnownScope) \| Unknown(String)`; `KnownScope` covers only `atproto` and the `transition:*` scopes, so `repo:site.standard.document` and `blob:image/*` go through as `Unknown` strings verbatim. The library does not parse or validate granular scopes, which is fine: the authorization server does. |
| Incremental scope upgrade (adding `repo:app.bsky.feed.post` later) | §5.7 | No dedicated API. A second `authorize` call with the larger scope set is the mechanism; whether the server treats it as an upgrade or a new grant is a server question, and the plan already anticipates the fallback of a second labelled authorization. |
| Session storage we control | §5.4, D16 | Yes. `SessionStore` and `StateStore` are traits over a generic `Store<K, V>`; a SQLite implementation is ours to write. Sessions are keyed by DID, one per account, which matches our model. |
| Our own outbound HTTP policy | §8 (SSRF) | Yes. Requests go through the `atrium_xrpc::HttpClient` trait, so the guarded client can be adapted rather than bypassed. |
| Client metadata served from our origin | §5.4 | Yes. Metadata types are provided; serving them at `/client-metadata.json` is trivial. |

## Assessment

The library covers the hard parts, PAR, DPoP, and refresh, that would otherwise
be several hundred lines of easy-to-get-subtly-wrong code, and it does not stand
in the way of granular scopes. Its version number (0.1.x) means breaking API
changes are likely; that is acceptable inside a workspace crate with no downstream
users (§6).

Two things to verify at implementation time rather than assume:

1. That the `Unknown` scope strings survive the PAR request unchanged and that
   `bsky.social`'s authorization server accepts `repo:site.standard.document`
   in the form the permissions spec prescribes (plan §4.4 open detail on
   action narrowing syntax).
2. Whether re-authorizing with a superset of scopes yields a session that
   replaces the stored one cleanly, since `SessionStore` is keyed by DID.

## Decision

**Adopt `atrium-oauth` for C3.1**, behind our own `oauth` module in
`eaten-at-atproto` so the rest of the app sees a small interface (`login
URL`, `callback`, `session for DID`, `logout`) and the dependency can be
swapped without touching routes. Write the SQLite `SessionStore` and
`StateStore` ourselves, and adapt `GuardedClient` to `atrium_xrpc::HttpClient`
so the SSRF policy applies to token and PAR requests too.

Fallback if the library proves unworkable during C3.1: implement PAR, DPoP
(ES256 through the same `jose` primitives, or `p256` + `jsonwebtoken`), and
refresh directly. The interface above is designed so that swap is contained.

## What C3.1 found (2026-09-09)

The two things to verify, and a few more:

1. **Granular scopes go through unchanged.** `Scope::Unknown` strings reach
   the PAR body verbatim (`scope=atproto repo:site.standard.document …`) and
   the local PDS at commit `7a23156ef` accepts
   `repo:<nsid>?action=create&action=update` and `blob:image/*`; the syntax
   matches `packages/oauth/oauth-scopes` in the atproto checkout.
2. **Scope upgrade** (§5.7) works by re-authorization (C4.2, 2026-09-09):
   the metadata declares every scope, sign-in requests a subset through
   `login_url_scoped`, and a later `authorize` with the full set replaces
   the stored session for the DID with one carrying the larger grant. The
   local PDS granted exactly the requested set. The old tokens are left
   to expire rather than revoked. The flip side: any later authorization
   replaces the session too, so a plain sign-in requesting the subset
   would drop the grant; the app requests the full set at sign-in for a
   DID whose stored session already has it.
3. **A failed token exchange panics** (`todo!()` in `OAuthClient::callback`).
   Our wrapper runs the exchange in a spawned task, so the panic is contained
   and becomes an error. Worth an upstream issue.
4. **Revocation expects HTTP 204**, but the atproto provider answers 200 with
   `{}`, so `revoke` always reports failure after succeeding. Our `logout`
   treats revocation as best effort and drops the local session regardless.
5. **Refresh is lazy**: the session registry refreshes only when
   `expires_at` has passed *and* a request came back 401 `invalid_token`. A
   token that is invalid but not yet expired by the clock is retried once
   with the same token and then fails. Acceptable for now; sign in again.
6. The resolver config types (`OAuthAuthorizationServerMetadataResolverConfig`
   and its sibling) are not exported, so they can only be spelled
   `Default::default()`.
7. The library's own HTTP client is not used (`default-features = false`);
   an adapter over `GuardedClient::send_raw` carries the `DPoP` and
   `DPoP-Nonce` headers both ways, and the nonce retry works through it.

The decision stands. The wrapper is `eaten-at-atproto::oauth`; nothing
outside it names an atrium type.
