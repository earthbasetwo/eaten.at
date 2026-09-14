## Contents

- [Ours: `at.eaten.*`](#ours-ateaten)
  - [`at.eaten.visit`](#ateatenvisit)
  - [`at.eaten.place`](#ateatenplace)
  - [`at.eaten.preferences`](#ateatenpreferences)
  - [What we write into the Standard document](#what-we-write-into-the-standard-document)
- [Upstream: `site.standard.*`](#upstream-sitestandard)
- [Other lexicons we touch](#other-lexicons-we-touch)
- [How ours are published](#how-ours-are-published)

## Ours: `at.eaten.*`

The shape of the whole design: we did not define a document record. A
write-up **is** a `site.standard.document`, so its publication, hosting,
theme, canonical URL, tags, labels, and Bluesky comment thread are
all the Standard record's own. What eaten.at adds is the document's
**`content`**: one `at.eaten.visit` object in that open union, and its
`$type` is what marks a document as ours (D23). A document with any other
content is an ordinary Standard post and we leave it alone.

Readers that do not know our type fall back to the document's
`textContent`, which we write as a plaintext rendering of the visit:
place, date, and verdict on the first line, then the prose.

Across all three schemas the same rules hold. `knownValues` is a
suggestion, not an enum: a value outside the list belongs to the authoring
client's own vocabulary and **MUST** be preserved on rewrite. Unknown
fields at every level are preserved too. Required fields are strict and
optional ones lenient: a malformed optional value from another client
reads as absent rather than sinking the document.

### `at.eaten.visit`

*File: `lexicons/at.eaten.visit.json` · type: object (not a record); a
`site.standard.document#content` member*

A visit to a place to eat: where, when, which meal, the author's verdict,
and the prose.

#### `main`

Required: `place`, `visitedOn`.

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `place` | ref [`at.eaten.place`](#ateatenplace) | | Where the author ate. |
| `visitedOn` | string | exactly 10 bytes, `YYYY-MM-DD` | The calendar date of the visit, in the place's local calendar. A date, not an instant: no time, no offset (D27). The PDS checks the length; the app checks the format. |
| `meal` | string | `knownValues`, ≤ 640 bytes | Which meal it was. Unknown values are preserved and shown as written. |
| `rating` | integer | 1–4 | The author's verdict on the house scale (D4). Absent means the author declined to rate. |
| `body` | open union | | The prose, keyed by `$type`. We write `at.markpub.markdown` (D12). Readers that know no member fall back to the document's `textContent`. |
| `photos` | array of [`#photo`](#photo) | ≤ 24 items | Photos of the visit, in the author's order (D37). The first is also written as the document's `coverImage` (D38). |

`meal` known values: `breakfast`, `brunch`, `lunch`, `dinner`, `lateNight`.

The rating scale, and how eaten.at renders it:

| Stored | Word | Marks |
| --- | --- | --- |
| 1 | Solid | `+` |
| 2 | Recommended | `++` |
| 3 | Strongly Recommended | `+++` |
| 4 | Can't Miss | `++++` |

It is an integer with a range rather than a string with `knownValues`
because the scale is ordinal and it is ours: a fifth value would have no
rendering. The PDS validates the range; a value outside it from another
client reads as unrated.

#### `#photo`

Required: `image`.

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `image` | blob | `image/*`, ≤ 1 000 000 bytes | The photo. eaten.at always re-encodes an upload as JPEG, so no camera metadata (the position above all) reaches the repository (D39). |
| `alt` | string | ≤ 2000 bytes, ≤ 1000 graphemes | Alt text, for people who do not see the image. |
| `aspectRatio` | [`#aspectRatio`](#aspectratio) | | The image's proportions, so a page can reserve its space. |

#### `#aspectRatio`

`width` and `height`, integers ≥ 1.

A malformed photo entry from another client is dropped on read; the
visit still renders.

### `at.eaten.place`

*File: `lexicons/at.eaten.place.json` · type: object defs only, no
record*

A place to eat, embedded in a visit. Its own lexicon so a later record (a
want-to-try entry, a list) can reference the same shape, the way
`site.standard.theme.color` is shared.

#### `main`

Required: `name`.

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `name` | string | ≤ 2000 bytes, ≤ 200 graphemes | The place's name, normally as Overture has it, editable by the author. |
| `address` | string | ≤ 3000 bytes, ≤ 300 graphemes | A one-line street address, normally as Overture has it, for display and a map link. |
| `price` | integer | 1–4 | Price band, 1 (cheapest) to 4, shown as that many currency signs. |
| `gersId` | string | ≤ 128 bytes | The place's Overture Maps GERS id, exactly as Overture issues it (D33). |
| `latE6`, `lonE6` | integer | ±90 000 000, ±180 000 000 | The place's position in microdegrees (degrees × 1 000 000), as Overture gives it. Lexicons have no float type; microdegrees are exact. |
| `urls` | array of [`#externalUrl`](#externalurl) | ≤ 12 items | Places to read more, in the author's preferred order. |

Two places are the same place when they share a `gersId`; a place
without one matches nothing. Names and addresses are for people and are
never used for identity. The app links a map at the coordinates when
both are present; an out-of-range coordinate reads as absent.

#### `#externalUrl`

Required: `url`.

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `url` | string | format `uri`, ≤ 2048 | The link itself. |
| `service` | string | `knownValues`, ≤ 640 | What this URL is. Consumers dispatch service-specific rendering on an **exact** match against `knownValues`. |
| `label` | string | ≤ 640 bytes, ≤ 64 graphemes | Human-readable link label. |

`service` known value: `officialSite`, the place's own website. Any
other link carries no `service` and is rendered as a plain link
labelled by the author's label or the URL's host, as is a value outside
the list (D30).

### `at.eaten.preferences`

*File: `lexicons/at.eaten.preferences.json` · type: record · key:
`literal:self`*

Per-user eaten.at settings, one record per repo, in the user's own
repository. All fields are optional; absence means defaults.

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `defaultPublication` | string, format `at-uri` | — | AT-URI of the account's eaten.at publication: the `site.standard.publication` every write-up is written to and a bare handle URL shows (D40). Absent, or naming a record that no longer exists, means the account has none yet; one is created on the first publish. The name is kept so more than one stays possible later. |
| `crosspostToBluesky` | boolean | `false` | Whether to default the Bluesky crosspost toggle on when publishing. A per-document override is always available. |
| `createdAt` | string, format `datetime` | — | When the record was first written. |

### What we write into the Standard document

Everything that is not the visit stays a Standard field, so a Standard
reader gets a complete post:

| Field | What we put there |
| --- | --- |
| `site` | The publication's AT-URI. |
| `title` | The author's, or the place's name when they gave none (D29). Standard requires one. |
| `description`, `tags`, `labels` | The author's own. Tags are free text and stay here, never in the visit (D26). |
| `coverImage` | Derived (D38): the first photo, when the visit has any, so Standard readers and unfurlers get a thumbnail. Never authored; with no photos, one another client set is carried over untouched (D32). |
| `path`, `publishedAt`, `updatedAt` | Set on publish; the path never changes. |
| `content` | The `at.eaten.visit`, with the markdown body inside it. |
| `textContent` | `<place> · <date>[ · <verdict>]`, a blank line, the prose as plaintext. |
| `bskyPostRef` | Set when a crosspost succeeds. |
| `links` | Not ours. Whatever another client put there is carried over untouched. |

## Upstream: `site.standard.*`

Standard's lexicons, which we read and write but do not own. Copies live in
`lexicons/upstream/` so the field constraints are checkable next to our
code; the authority is Standard.

### `site.standard.document`

*Record, key `tid`.* A published article, blog post, or other content;
belongs to a publication or exists on its own. Required: `site`, `title`,
`publishedAt`.

| Field | Type | Description |
| --- | --- | --- |
| `site` | string, format `uri` | A publication record (`at://`) or a publication URL (`https://`) for loose documents. No trailing slash. |
| `title` | string, ≤ 5000 bytes / 500 graphemes | Title of the document. |
| `path` | string | Combined with the site or publication URL to make the canonical URL. Leading slash. |
| `publishedAt` | string, format `datetime` | Publish time. |
| `updatedAt` | string, format `datetime` | Last edit. |
| `description` | string, ≤ 30000 bytes / 3000 graphemes | Brief description or excerpt. |
| `content` | open union | The document's content, each entry keyed by `$type`; extensible by other lexicons for other formats. **This is where `at.eaten.visit` goes.** |
| `textContent` | string | Plaintext rendering of the content, no markdown or other formatting. |
| `coverImage` | blob, `image/*`, < 1 MB | Thumbnail or cover image. |
| `tags` | array of string, each ≤ 1280 bytes / 128 graphemes | Tags or categories. No leading hashtags. |
| `links` | union | Relationships between this document and external resources. Not used by eaten.at. |
| `labels` | union of `com.atproto.label.defs#selfLabels` | Self-labels; effectively content warnings. |
| `bskyPostRef` | ref `com.atproto.repo.strongRef` | Strong reference to a Bluesky post, for off-platform comments. We set this when a crosspost succeeds. |
| `contributors` | array of `#contributor` | Additional credited people. |

`#contributor` — required `did` (format `did`), plus optional `role` and
`displayName` (each ≤ 1000 bytes / 100 graphemes).

### `site.standard.publication`

*Record, key `tid`.* A blog, website, or content platform: the container
for documents, and where branding lives. Required: `url`, `name`.

| Field | Type | Description |
| --- | --- | --- |
| `url` | string, format `uri` | Base publication URL. The canonical document URL is this plus the document `path`. |
| `name` | string, ≤ 5000 bytes / 500 graphemes | Name of the publication. |
| `description` | string, ≤ 30000 bytes / 3000 graphemes | Brief description. |
| `icon` | blob, `image/*`, < 1 MB | Square identifying image, at least 256×256. |
| `basicTheme` | ref `site.standard.theme.basic` | Simplified theme for tools and apps displaying the content. |
| `labels` | union of `com.atproto.label.defs#selfLabels` | Self-labels. |
| `preferences` | ref `#preferences` | Platform-specific preferences. |

`#preferences` — `showInDiscover` (boolean, default `true`): whether the
publication appears in discovery feeds.

### `site.standard.theme.basic`

*Record, key `tid`.* Basic color customization for a publication. All four
fields are required, each a union over
`site.standard.theme.color#rgb`: `background` (content background),
`foreground` (content text), `accent` (links and button backgrounds), and
`accentForeground` (button text).

### `site.standard.theme.color`

*Object definitions only, no record.* `#rgb` requires `r`, `g`, `b`,
integers 0–255. `#rgba` adds `a`, an integer 0–100.

### `site.standard.graph.recommend`

*Record, key `tid`.* Declares a recommendation of a document. Required:
`document` (AT-URI of the `site.standard.document`) and `createdAt`.

### `site.standard.graph.subscription`

*Record, key `tid`.* Declares a subscription to a publication. Required:
`publication` (AT-URI of the `site.standard.publication`); optional
`createdAt`.

## Other lexicons we touch

Not in `lexicons/`, but part of the wire contract.

| NSID | Direction | What we do with it |
| --- | --- | --- |
| `at.markpub.markdown` | write | The `body` union entry we put in every visit: `flavor` `"commonmark"` and `text`, itself an `at.markpub.text` with the `markdown` string. |
| `at.markpub.text` | write | The inner object holding the markdown source. |
| `app.bsky.feed.post` | write | The opt-in crosspost. Text ≤ 300 graphemes, with an `app.bsky.embed.external` card pointing at the document. |
| `app.bsky.embed.external` | write | The link card on that post: `uri`, `title`, `description`, optional `thumb` blob. |
| `com.atproto.repo.strongRef` | write | Shape of `bskyPostRef` on the document once a crosspost exists. |
| `com.atproto.label.defs#selfLabels` | write | Self-labels on documents and publications. |
| `app.bsky.feed.getPostThread` | read | Unauthenticated AppView query that fetches the comment thread for a document's `bskyPostRef`. We handle `#threadViewPost`, `#notFoundPost`, `#blockedPost`, and unknown `$type`s. |
| `com.atproto.lexicon.schema` | write | The record type our own schemas are published as (below). |

## How ours are published

The three `at.eaten.*` schemas are published to the atproto network as
`com.atproto.lexicon.schema` records in the authority's repo, so any client
can resolve the NSID and read the definition. The tool is
`crates/eaten-at/src/bin/publish_lexicons.rs`:

```text
publish-lexicons --dry-run     # diff files against the repo; exit 1 on drift
publish-lexicons               # write whatever differs
publish-lexicons --verify      # resolve each NSID via DNS and compare
```

Credentials come from `EATEN_AT_LEXICON_IDENTIFIER` (handle or DID) and
`EATEN_AT_LEXICON_APP_PASSWORD`; a dry run needs only the identifier.
The development settings apply, so this also runs against a local PDS.

`--dry-run` in CI is what keeps `lexicons/*.json` and the published records
from drifting apart. The tool creates and updates but never deletes: the
retired `at.eaten.subject` was never published from this repository, so
there is nothing to remove; if a copy exists in the authority's repo from
before, delete that record by hand once.
