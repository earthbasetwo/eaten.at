# 08 · One publication per account

Feedback 12: *Remove the ability to have multiple publications for v1.
Every user using eaten.at should have a single designated eaten.at
publication.*

Feedback 16 in part: *The settings page should be tertiary priority.*
Settings shrinks to one publication here; plan 11 puts it last on the
landing page.

Also closes feedback 11 (a separate new-publication flow), which the
first round deferred: with one publication there is no such flow. Every
later plan in this round assumes this one.

## What "one publication" means

Standard lets a repository hold any number of `site.standard.publication`
records, and other Standard apps (Leaflet, for one) create their own. We
cannot stop that, and should not read around it. What we can do is
**designate one of them as the account's eaten.at publication** and
make every authoring path write to it.

The designation already exists: `at.eaten.preferences.defaultPublication`
(plan §4.3). Today it is a hint, "the one to preselect in the editor",
and the editor still offers the rest plus "A new publication…". After
this plan it is the whole story:

- **The eaten.at publication** is the one the preferences record names.
  A record that names nothing, or names a publication that no longer
  exists, means the account has none yet.
- **Authoring** always writes to it. The editor has no publication
  select and no new-publication fields. A first publish creates the
  publication (below), writes the preference, then the document, in that
  order.
- **Reading** is unchanged for repositories that are not ours: `/at/{did}/`
  still resolves through the preference, then a lone publication, then
  the chooser (D10's reader side stays; it costs nothing and lets a
  Leaflet author be read here). The chooser is never shown for a
  repository that has an eaten.at designation.
- **Settings** is one publication's settings: its name, its description,
  and where it lives (hosted subdomain or own domain, as today).

## Creating the publication

Created lazily, by whichever of these happens first: the first publish,
or the first save on the settings page. Nothing is written at sign-in;
an account that signs in and leaves has no records from us.

One function owns it, `publish::ensure_publication(state, identity,
session) -> Result<Record<Publication>, PublishError>`:

1. Read the preference. If it names a publication that exists, return it.
2. Otherwise create the record with the defaults below, write the
   preference naming it (`createdAt` set now), evict the publication
   list and the preference from the cache, and return it.

**Defaults.** Standard requires `name` and `url`.

- `name`: the account's handle as text, `alice.bsky.social`, or the
  DID when there is no verified handle. Plain, correct, and changed in
  a minute on the settings page. The alternative, fetching the Bluesky
  display name from the AppView, adds a network call to a publish for a
  guess the author may not want on a nameplate.
- `url`: when the deployment can host subdomains
  (`can_host_subdomains`), a hosted subdomain claimed for the author:
  the first label of the handle (`alice` from `alice.bsky.social`,
  `rosslebeau` from `rosslebeau.com`) passed through `validate_name`,
  with `-2`, `-3`, … appended while the name is taken or reserved. The
  claim is made before the record is written and released if the write
  fails, the same order the settings page keeps today. When the
  deployment cannot host (local development), `url` is the site route
  of the publication itself, `{public_url}/at/{did}/{rkey}/`.

The second default needs the record key before the record exists, so
the app mints the TID itself and passes it to `createRecord` (the
crosspost path already supplies its own `rkey`). `eaten-at-atproto`
gains `Tid::now()`: 64 bits of microsecond timestamp and a clock id in
base32-sortable, as the PDS mints them. Known limit of the fallback URL:
a document's canonical link is `url + path`, and the site route serves
documents by record key, not path, so on a non-hosting deployment the
canonical link does not resolve. That is today's behaviour for a
publication served by the author, only now it also shows up in local
development; it is noted, not fixed, here.

## The editor and publish path

- `EditorForm` loses `publication`, `new_publication_name`, and
  `new_publication_url`; `PUBLICATION_NEW`, `PublicationOption`, and the
  `publication()` fieldset go; `carried()` stops carrying them.
- `Context.publications` and `Target` go from `editor/draft.rs`; the
  draft has no target. Validation loses the publication rules and their
  tests.
- `publish::publish` calls `ensure_publication` and uses its URI as
  `site`. `update_preferences` shrinks to the crosspost default only,
  since the designation is written by `ensure_publication`.
- `routes/write.rs`'s `Author` loses `publications` and `default`.
- An edit keeps the document's existing `site`, whatever it is: a
  document written to another publication by an earlier build or
  another client is not moved.

## Settings

`/settings` becomes one card, "Your publication":

- Kicker "Settings", `h1` "Your publication", lede "Its name, what it
  says about itself, and where it lives."
- No publication yet: the card shows the defaults that would be used
  ("It will be called alice.bsky.social at alice.eaten.at") with the
  same fields, and the first Save creates it through
  `ensure_publication` and then applies the form. Copy under the
  fields: "Made when you save, or when you publish your first write-up."
- Fields: Name (required), Description (optional, a short textarea),
  then the hosting choice exactly as today (hosted subdomain or own
  `https` URL). One primary "Save".
- Route: `POST /settings` replaces `POST /settings/{pub_rkey}/hosting`.
  The record is rewritten once with name, description, and url; the
  claim logic and the restore-on-refusal path move over unchanged.
- The theme stays out of scope (it is not in the feedback).

## Touchpoints

- `lexicons/at.eaten.preferences.json` and `docs/lexicons.md`: the
  description of `defaultPublication` changes from "a hint" to "the
  account's eaten.at publication". The field keeps its name (settled
  below).
- `read.rs`: `choose_publication` gains a sibling, `own_publication
  (identity) -> Option<Record<Publication>>`, the designation and
  nothing else, for the editor, settings, and plan 11's landing page.
- `publish.rs`: `ensure_publication`; `update_preferences` shrinks.
- `eaten-at-atproto`: `Tid::now()` with a test that keys sort by time
  and parse as 13 base32-sortable characters.
- `editor/form.rs`, `editor/draft.rs`, `editor/view.rs`,
  `routes/write.rs` as above; `hosting.rs` for the label derivation
  (`suggest_name(handle) -> String` and the `-n` loop against
  `claims()`).
- `routes/settings.rs`: the one-card page and `POST /settings`.
  `app.rs` routes.
- `scripts/dev-env.mjs`: the seed keeps two publications (the themed one
  proves theming) and the preferences record naming the first, which is
  exactly the designated shape. Add a second seeded account with no
  publication so the "not yet" states can be rendered.
- `scripts/visual-check.mjs`: `settings` (with a publication),
  `settings-none` and `write` for the account without one (needs a
  second dev session cookie), `write-publish-first` if the stub PDS
  allows a publish end to end (it does; the seed already writes).
- `docs/decisions.md`: D40 one designated publication per account
  (supersedes the "users may have many" half of D10 for authoring);
  D41 lazy creation with the handle-derived defaults.
- `docs/design.md`: the settings recipe, the editor's Details group
  without the publication select.
- README: routes table (`/settings`), the D10 sentence in the routes
  description.
- Tests: `draft.rs` (publication rules removed), `publish.rs`
  (`ensure_publication` creates once and is idempotent, claim released
  on a refused write, defaults from a handle and from a DID-only
  identity), `routes.rs` (settings creates, edit keeps a foreign
  `site`), `hosting.rs` (label suggestion: `alice.bsky.social` →
  `alice`, `rosslebeau.com` → `rosslebeau`, a reserved word → `-2`,
  a 70-character label truncated to 63).

## Definition of done

- The editor never asks which publication; a first publish creates the
  account's publication and lands the document in it.
- `/at/{did}/` for an eaten.at account goes straight to that
  publication; a foreign repository still reads as before.
- Settings edits the one publication's name, description, and address,
  and can create it before any write-up exists.
- `just check` and `just visual-check` pass.

## As built (2026-09-13)

As planned, with these notes.

- The settings page takes the whole spec on a first save (name,
  description, and the address as typed), so creating there is one
  write plus the preference, not a create with defaults followed by a
  rewrite. `publish::create_publication` takes a `PublicationSpec`
  whose `Home` is one of: this exact hosted label, this label or the
  next free numbered one (the publish path's default), an own `https`
  origin, or the publication's site route (the default where the
  deployment cannot host).
- A server that ignores the record key the app asked for (the test mock
  does) is tolerated: the claim is moved to the key actually written
  and a warning is logged.
- The `PublishError` gained a `Home(String)` variant for a hosted name
  that could not be claimed. The editor shows it as a form error that
  points at settings; the settings page shows it beside the subdomain
  field.
- A refused write on the publication's `createRecord` releases the
  claim; a refused preference write after a successful create does not
  undo the create (the record is real), and the next publish would make
  a second one. Rare enough to log rather than compensate for.
- `Published` now carries the document's `site` as written and derives
  its page path, so an edit of a document whose `site` is not one of
  our records still redirects somewhere sensible (the repo root).
- The visual check's `write-publish-first` was not added: a `dev-session`
  cookie has no OAuth tokens, so a publish from the browser fails as an
  expired sign-in would. The route tests cover the create path against
  the mock PDS instead.
- Not done here: the changed lexicon description still needs
  `just lexicons-publish`, as before.

## Decisions

Settled 2026-09-13:

1. **`defaultPublication` keeps its name.** Only the description
   changes. The name leaves the door open to more than one publication
   later at no cost now; renaming would have bought nothing.
2. **The default name is the handle as text** (the DID when there is
   no verified handle). Not the Bluesky display name (a network call at
   publish for a name the author did not choose) and not a question on
   first publish (a step the feedback wants gone).

Accepted as recommended, not separately discussed:

3. **Creation is lazy**: at first publish or first settings save,
   whichever comes first. Not in the OAuth callback, which would write
   three records before the author has done anything and put PDS
   latency in the sign-in.
4. **The chooser stays for foreign repositories.** It is built, and it
   is the only way to read a multi-publication Standard author here.
   It is never shown for a repository with an eaten.at designation.
