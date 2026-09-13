## Contents

- [Ours: `at.eaten.*`](#ours-ateaten)
  - [`at.eaten.subject`](#ateatensubject)
  - [`at.eaten.preferences`](#ateatenpreferences)
- [Upstream: `site.standard.*`](#upstream-sitestandard)
- [Other lexicons we touch](#other-lexicons-we-touch)
- [How ours are published](#how-ours-are-published)

## Ours: `at.eaten.*`

The shape of the whole design: we did not define a document record. A
write-up **is** a `site.standard.document` (any Standard-aware reader shows
it) with one extra object in the document's open `links` union that says
what it is about. Our other record holds settings, so a client that never
heard of us still renders the writing.

The subject is a **placeholder** carried over from the fork: a title and
links. eaten.at's own plan decides what a subject really is (D11 in
`docs/decisions.md`) and extends this schema.

### `at.eaten.subject`

*File: `lexicons/at.eaten.subject.json` · type: object (not a record)*

Identifies what a `site.standard.document` is about. It is placed in the
document's `links` open union, and **its presence is what marks a document
as one of ours**: no marker field, no separate collection. A document
without it is an ordinary Standard post and we leave it alone.

#### `main`

Required: `title`.

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `title` | string | ≤ 2000 bytes, ≤ 200 graphemes | The subject's name, as the author gives it. |
| `externalUrls` | array of [`#externalUrl`](#externalurl) | ≤ 12 items | Places to read more about the subject, in the author's preferred order. |

#### `#externalUrl`

Required: `url`.

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `url` | string | format `uri`, ≤ 2048 | The link itself. |
| `service` | string | `knownValues`, ≤ 640 | Service this URL points at. Consumers dispatch service-specific rendering on an **exact** match against `knownValues`. |
| `label` | string | ≤ 640 bytes, ≤ 64 graphemes | Human-readable link label. |

`service` known values: `officialSite`.

`knownValues` is a suggestion, not an enum. A value outside the list
belongs to the authoring client's own vocabulary: it **MUST** be preserved
on rewrite and rendered as a plain link. That rule is why our editor round-
trips documents it did not write without losing anything.

### `at.eaten.preferences`

*File: `lexicons/at.eaten.preferences.json` · type: record · key:
`literal:self`*

Per-user eaten.at settings, one record per repo, in the user's own
repository. All fields are optional; absence means defaults.

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `defaultPublication` | string, format `at-uri` | — | AT-URI of the `site.standard.publication` to show for a bare handle URL and to preselect in the editor. A hint, not a restriction: the user may write documents to any publication in their repo. |
| `crosspostToBluesky` | boolean | `false` | Whether to default the Bluesky crosspost toggle on when publishing. A per-document override is always available. |
| `createdAt` | string, format `datetime` | — | When the record was first written. |

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
| `content` | open union | The document's content, each entry keyed by `$type`; extensible by other lexicons for other formats. |
| `textContent` | string | Plaintext rendering of the content, no markdown or other formatting. |
| `coverImage` | blob, `image/*`, < 1 MB | Thumbnail or cover image. |
| `tags` | array of string, each ≤ 1280 bytes / 128 graphemes | Tags or categories. No leading hashtags. |
| `links` | union | Relationships between this document and external resources. **This is where `at.eaten.subject` goes.** |
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
| `at.markpub.markdown` | write | The `content` union entry we put in every document: `flavor` `"commonmark"` and `text`, itself an `at.markpub.text` with the `markdown` string. `textContent` carries the plaintext beside it. |
| `at.markpub.text` | write | The inner object holding the markdown source. |
| `app.bsky.feed.post` | write | The opt-in crosspost. Text ≤ 300 graphemes, with an `app.bsky.embed.external` card pointing at the document. |
| `app.bsky.embed.external` | write | The link card on that post: `uri`, `title`, `description`, optional `thumb` blob. |
| `com.atproto.repo.strongRef` | write | Shape of `bskyPostRef` on the document once a crosspost exists. |
| `com.atproto.label.defs#selfLabels` | write | Self-labels on documents and publications. |
| `app.bsky.feed.getPostThread` | read | Unauthenticated AppView query that fetches the comment thread for a document's `bskyPostRef`. We handle `#threadViewPost`, `#notFoundPost`, `#blockedPost`, and unknown `$type`s. |
| `com.atproto.lexicon.schema` | write | The record type our own schemas are published as (below). |

## How ours are published

The two `at.eaten.*` schemas are published to the atproto network as
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
from drifting apart.
