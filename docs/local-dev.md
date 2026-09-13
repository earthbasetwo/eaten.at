# Local development against a real atproto stack

The tests cover the protocol logic with mocks. To see the app, the publish
tool, and later the OAuth flow run against a real PLC directory, PDS, and
handle resolution without touching a public domain, run a local atproto
network and point the app at it.

## Layout

Keep one checkout of `bluesky-social/atproto` next to this repository,
shared by every atproto project on the machine:

```
~/Projects/earthbasetwo/atproto        # the atproto monorepo
~/Projects/earthbasetwo/eaten-at   # this repo
```

Nothing in that checkout is specific to this project. The local network runs
in memory and forgets everything on restart, so the project-specific part is
the seed, and that lives here in `scripts/dev-env.mjs`.

## One-time setup

Needs Node 22 or newer and pnpm 11 (the checkout pins pnpm in
`package.json`; `.nvmrc` says 24). Docker is **not** needed: the runner
starts only the PLC and PDS, not the AppView.

On Node 25 the build fails with `localStorage.getItem is not a function`,
because Node 25 turned on a web-storage global that a build-time dependency
mishandles. Turning it off is enough; `just dev-env` does the same for the
runner:

```
cd ../atproto
make deps
NODE_OPTIONS=--no-experimental-webstorage make build
```

Installing Node 24 instead also works, and is what the checkout expects.

The build takes several minutes the first time. Pin the commit that worked:

| Date | atproto commit |
|---|---|
| 2026-09-07 | `7a23156ef` |

Pull that checkout deliberately, not by habit; the dev-env tracks `main`.

## Every session

Terminal 1, from this repo:

```
just dev-env
```

This starts a PLC on `localhost:2582` and a PDS on `localhost:2583`, creates
two accounts, seeds records, and writes `.env.dev`:

- **`eaten.test`**, the lexicon publisher, with an app password.
- **`alice.test`**, an author with a themed publication ("Field Notes"),
  three write-ups carrying an `at.eaten.subject` (one with a description,
  one with a Bluesky post reference so the comment section renders), and
  one plain document without a subject, to prove the filtering.

Set `ATPROTO_DIR` if the checkout is somewhere other than `../atproto`.

Terminal 2:

```
just run-dev                  # the app, with .env.dev loaded
just lexicons-check-dev       # dry run: our two schemas against eaten.test's repo
just lexicons-publish-dev     # publish, then verify via _lexicon.eaten.at
```

Then open `http://127.0.0.1:3000/@alice.test`.

To sign in, open `http://127.0.0.1:3000/login` and enter `alice.test`; the
PDS's own page asks for the password (`dev-password`) and consent, then
returns to the app. Because the public URL is a plain-HTTP loopback address
the app registers as an atproto *loopback client*: the PDS derives the
client metadata from the `client_id` itself and no signing key is needed.
Sessions live in `.dev-cache.db` and survive `just run-dev` restarts; a new
`just dev-env` wipes them along with everything else.

Restarting the network means re-running `just dev-env`; it rewrites
`.env.dev` with the new DIDs and deletes `.dev-cache.db`, the app's cache for
dev runs. Both files are git-ignored. Without that wipe the app would keep
serving the previous network's DIDs from its cache (identities are cached
for a day, handle lookups for an hour) and every page would fail with
"Could not find repo". Restart `just run-dev` after restarting the network.

## What `.env.dev` contains

| Variable | Effect |
|---|---|
| `EATEN_AT_DEV_INSECURE=1` | The outbound HTTP policy allows plain HTTP and loopback. This disables the SSRF guards; the app logs a warning at startup. Never set it in production. |
| `EATEN_AT_PLC_DIRECTORY` | Replaces `https://plc.directory` with the local PLC. |
| `EATEN_AT_DEV_HOSTS` | Maps `eaten.test` and `alice.test` to the local PDS before the system resolver is consulted, so handle verification via `/.well-known/atproto-did` reaches it. |
| `EATEN_AT_DEV_DNS_TXT` | A fixed TXT answer for `_lexicon.eaten.at`, standing in for the real DNS record. The name comes from the NSID authority, not from the publisher's handle, so it is the production name even locally. The first `=` separates name from value. |
| `EATEN_AT_LEXICON_*` | Credentials for the publish tool. |
| `EATEN_AT_DEV_PDS`, `EATEN_AT_DEV_ALICE_DID` | Used by the `just` recipes and handy for `curl`. |
| `EATEN_AT_DB=.dev-cache.db` | A separate database for dev runs (cache, OAuth state, sessions), wiped by the runner on every start. |

Not overridden: `EATEN_AT_BSKY_APPVIEW`. The local network has no AppView, so comment threads are read from the public one; a seeded document gets a thread by adding a `bskyPostRef` naming a real Bluesky post to its record (`putRecord` on the local PDS).

Not set in `.env.dev`: `EATEN_AT_OAUTH_KEY_FILE`. In production it names
the private JWK that makes the app a confidential OAuth client; the app
creates the file on first start if it is missing. A loopback client has no
use for one.

Any name not in the overrides falls through to the real resolvers.

## What this does not exercise

- Your DNS provider and propagation. Rehearse that once on a throwaway
  subdomain of a domain you already own, with a throwaway bsky.social account
  and temporarily renamed schema IDs under that subdomain's authority.
- TLS, wildcard certificates, and the reverse proxy: deployment concerns
  for hosted subdomains.
- The Bluesky AppView (comment-thread reading). The full `make run-dev-env`
  in the atproto checkout provides one, and needs Docker for its Postgres and
  Redis.
