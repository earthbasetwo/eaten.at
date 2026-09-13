#!/usr/bin/env node
// Start a local atproto network (PLC + PDS, no AppView, no Docker) from a
// sibling checkout of bluesky-social/atproto, seed it with a lexicon
// publisher account and an author with a few visits, and write the
// matching EATEN_AT_* variables to .env.dev.
//
//   ATPROTO_DIR=../atproto node scripts/dev-env.mjs
//
// PLC_PORT, PDS_PORT, and PLACES_PORT override the ports (default 2582,
// 2583, and 2584), so a second local network can run beside another
// project's. The third is a stub of the Open Places API with canned
// results, so the editor's place search works offline.
//
// The network is in memory: everything is recreated on each start. Stop
// with Ctrl-C.

import { rm, writeFile } from 'node:fs/promises'
import http from 'node:http'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const ROOT = path.resolve(HERE, '..')
const ATPROTO_DIR = path.resolve(ROOT, process.env.ATPROTO_DIR ?? '../atproto')
const PLC_PORT = Number(process.env.PLC_PORT ?? 2582)
const PDS_PORT = Number(process.env.PDS_PORT ?? 2583)
const PLACES_PORT = Number(process.env.PLACES_PORT ?? 2584)
const PASSWORD = 'dev-password'
// The app's cache for dev runs. Wiped on every start: the network's DIDs
// change each time, and cached identities would point at repos that no
// longer exist.
const DEV_CACHE = '.dev-cache.db'

const devEnv = await import(
  pathToFileURL(path.join(ATPROTO_DIR, 'packages/dev-env/dist/index.js')).href
).catch((err) => {
  console.error(`Could not load @atproto/dev-env from ${ATPROTO_DIR}.`)
  console.error('Run `make deps && make build` in that checkout first, or set ATPROTO_DIR.')
  console.error(err.message)
  process.exit(1)
})

for (const suffix of ['', '-wal', '-shm']) {
  await rm(path.join(ROOT, DEV_CACHE + suffix), { force: true })
}

const network = await devEnv.TestNetworkNoAppView.create({
  plc: { port: PLC_PORT },
  pds: { port: PDS_PORT, hostname: 'localhost' },
})
const pds = network.pds

async function account(handle) {
  const agent = pds.getAgent()
  await agent.createAccount({ handle, email: `${handle}@example.invalid`, password: PASSWORD })
  const appPassword = await agent.com.atproto.server.createAppPassword({ name: 'eaten-at' })
  return { handle, did: agent.did, agent, appPassword: appPassword.data.password }
}

const rgb = (r, g, b) => ({ $type: 'site.standard.theme.color#rgb', r, g, b })

async function createRecord(agent, collection, record) {
  const res = await agent.com.atproto.repo.createRecord({ repo: agent.did, collection, record })
  return res.data
}

const publisher = await account('eaten.test')
const alice = await account('alice.test')

// Two publications: Field Notes has no theme, so it renders in the site's
// own palette; After Hours carries an author theme, so the theme path (and
// its contrast clamp) is exercised too. A dark ground is the harder case:
// the raised and sunken surfaces have to move the other way.
const { uri: publicationUri } = await createRecord(alice.agent, 'site.standard.publication', {
  $type: 'site.standard.publication',
  url: 'https://alice.eaten.test',
  name: 'Field Notes',
  description: 'Write-ups by Alice, one visit at a time.',
})
const { uri: themedPublicationUri } = await createRecord(alice.agent, 'site.standard.publication', {
  $type: 'site.standard.publication',
  url: 'https://afterhours.alice.eaten.test',
  name: 'After Hours',
  description: 'Late-night write-ups, in the author\'s own colours.',
  // Each colour is a union member and the PDS validates it, so $type is required.
  basicTheme: {
    $type: 'site.standard.theme.basic',
    background: rgb(34, 24, 31),
    foreground: rgb(110, 96, 104), // fails contrast on purpose
    accent: rgb(236, 128, 96),
    accentForeground: rgb(34, 24, 31),
  },
})
// With two publications, a bare handle URL would show a chooser; the
// preference sends it to Field Notes, the page people open first.
await alice.agent.com.atproto.repo.putRecord({
  repo: alice.did,
  collection: 'at.eaten.preferences',
  rkey: 'self',
  record: {
    $type: 'at.eaten.preferences',
    defaultPublication: publicationUri,
    createdAt: new Date().toISOString(),
  },
})

// A Bluesky post to hang a comment thread on. The PDS validates the
// strong ref's CID, so it has to be a real record.
const { uri: postUri, cid: postCid } = await createRecord(alice.agent, 'app.bsky.feed.post', {
  $type: 'app.bsky.feed.post',
  text: 'New write-up: Noodle House.',
  createdAt: '2026-07-28T13:00:00.000Z',
})

// Plain text for `textContent`: what a reader that does not know
// at.eaten.visit sees. Markdown syntax stripped, one paragraph per line,
// with the place, date, and verdict first, the way the app writes it.
// Good enough for a seed; the app derives its own excerpts.
const RATING_WORDS = { 1: 'Solid', 2: 'Recommended', 3: 'Strongly Recommended', 4: 'Can’t Miss' }
const plain = (md) =>
  md
    .replace(/^#+\s+/gm, '')
    .replace(/^>\s?/gm, '')
    .replace(/^[-*]\s+/gm, '')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/[*_`]/g, '')
    .replace(/\n{2,}/g, '\n')
    .trim()
const textContent = (d) =>
  [
    [d.place.name, d.visitedOn, d.rating && RATING_WORDS[d.rating]].filter(Boolean).join(' · '),
    plain(d.markdown),
  ]
    .filter(Boolean)
    .join('\n\n')

// Placeholder visits. Enough to see a listing with ratings, a document
// with a description and one whose excerpt is derived, an unrated visit,
// a comment section, tags, a GERS id, and links out; the prose is invented for
// the seed.
const docs = [
  {
    title: 'A first write-up',
    path: '/2026/09/noodle-house',
    publishedAt: '2026-09-07T12:00:00.000Z',
    tags: ['noodles', 'one long sit'],
    description: 'What a write-up with a place, a rating, an excerpt, tags, and a comment thread looks like.',
    place: {
      name: 'Noodle House',
      address: '12 Example Lane',
      price: 2,
      gersId: '08f2a5b6c7d8e9f0a1b2c3d4e5f60718',
      latE6: 40688838,
      lonE6: -73979914,
      urls: [
        { url: 'https://example.com/noodle-house', service: 'officialSite' },
        { url: 'https://example.com/noodle-house/menu', service: 'menu' },
        { url: 'https://example.com/noodle-house/elsewhere', service: 'shop', label: 'Elsewhere' },
      ],
    },
    visitedOn: '2026-09-06',
    meal: 'dinner',
    rating: 3,
    bskyPostRef: { uri: postUri, cid: postCid },
    markdown: `# A first write-up

This is the newest document in the seed, so it leads the listing. It has an
author-written *description*, which the listing and the link previews use
as the excerpt.

## What to look for

- The visit card above this prose: the place's name, then the date, the
  meal, the price band, the address, and the rating as plus signs.
- Links in the footer: one labelled from its known service, one by its
  host (its \`menu\` service is another client's word, kept but not ours),
  one by the label the author gave it, and a map link to OpenStreetMap
  from the place's coordinates.
- A comment section, because the record names a Bluesky post. The local
  network has no AppView, so it stays empty here.`,
  },
  {
    title: 'A second write-up',
    path: '/2026/08/corner-cafe',
    publishedAt: '2026-08-15T09:30:00.000Z',
    tags: ['coffee'],
    place: {
      name: 'Corner Café',
      price: 1,
      gersId: '08f2a5b6c7d8e9f0a1b2c3d4e5f60719',
      urls: [{ url: 'https://example.com/corner-cafe', service: 'officialSite' }],
    },
    visitedOn: '2026-08-14',
    meal: 'brunch',
    rating: 4,
    markdown: `This one has no description, so the listing derives its excerpt from the
first paragraph of the body, cut at a sentence boundary.

> A blockquote, to show the hairline rule the prose uses for one.

And a second paragraph that the excerpt never reaches.`,
  },
  {
    title: 'A third write-up',
    path: '/2026/07/the-old-mill',
    publishedAt: '2026-07-28T13:00:00.000Z',
    tags: ['long read'],
    place: { name: 'The Old Mill', address: '1 Mill Road' },
    visitedOn: '2026-07-27',
    markdown: `The oldest of the three, unrated and with no links out at all: the footer
shows only the tags, the feed, and the author.

Tags are the author's own words. \`long read\` is one here, matched
loosely on the tag page, so \`Long  Read\` finds it too.`,
  },
]
// The themed publication's visits: enough for a listing, a document, and
// a tag page in the author's colours.
const themedDocs = [
  {
    site: themedPublicationUri,
    title: 'A themed write-up',
    path: '/2026/09/night-market',
    publishedAt: '2026-09-05T22:00:00.000Z',
    tags: ['late'],
    description: 'The same pages as Field Notes, recoloured by the publication\'s theme.',
    place: {
      name: 'Night Market',
      price: 1,
      gersId: '08f2a5b6c7d8e9f0a1b2c3d4e5f6071a',
      latE6: 51507351,
      lonE6: -127758,
      urls: [{ url: 'https://example.com/night-market', service: 'officialSite' }],
    },
    visitedOn: '2026-09-05',
    meal: 'lateNight',
    rating: 2,
    markdown: `This publication declares a dark theme whose text colour fails contrast
on purpose, so the page shows the clamped colours, not the author's.

> Cards, fields, and panels take their surfaces from the theme too.

Everything else, the type, the spacing, the shapes, stays the site's.`,
  },
  {
    site: themedPublicationUri,
    title: 'Another themed write-up',
    path: '/2026/08/the-diner',
    publishedAt: '2026-08-20T23:30:00.000Z',
    tags: ['late', 'short'],
    place: { name: 'The Diner' },
    visitedOn: '2026-08-20',
    rating: 1,
    markdown: `A second entry, so the listing has more than one card.`,
  },
]
// Created oldest first so record keys (TIDs) run in publish order, as they
// would for a real author; listings page through keys, newest first.
const docUris = []
for (const d of [...docs, ...themedDocs].sort((a, b) => a.publishedAt.localeCompare(b.publishedAt))) {
  const { uri } = await createRecord(alice.agent, 'site.standard.document', {
    $type: 'site.standard.document',
    site: d.site ?? publicationUri,
    title: d.title,
    path: d.path,
    publishedAt: d.publishedAt,
    tags: d.tags,
    ...(d.description ? { description: d.description } : {}),
    ...(d.bskyPostRef ? { bskyPostRef: d.bskyPostRef } : {}),
    content: {
      $type: 'at.eaten.visit',
      place: d.place,
      visitedOn: d.visitedOn,
      ...(d.meal ? { meal: d.meal } : {}),
      ...(d.rating ? { rating: d.rating } : {}),
      body: {
        $type: 'at.markpub.markdown',
        flavor: 'commonmark',
        text: { $type: 'at.markpub.text', markdown: d.markdown },
      },
    },
    textContent: textContent(d),
  })
  docUris.push(uri)
}
// A document that is not a visit in the same publication, to prove filtering.
await createRecord(alice.agent, 'site.standard.document', {
  $type: 'site.standard.document',
  site: publicationUri,
  title: 'A plain post',
  publishedAt: '2026-09-01T12:00:00.000Z',
  textContent: 'Just a blog post.',
})

// A stand-in for the Open Places API (GET /v1/places): the same three
// places whatever the query, at the shape the real API answers with. A
// query containing "nothing" finds nothing; one containing "quota" is
// refused as over quota, so the editor's unavailable state can be seen.
const PLACES = [
  {
    place_id: 'overture:76f1250d-8e38-40b3-a021-bfe1c16b4e1c',
    name: 'Noodle House', lat: 40.701607, lon: -73.986565, distance_mi: 0.4,
    category: 'noodle_restaurant', categories: ['noodle_restaurant'],
    address: { formatted: '12 Example Lane', street: '12 Example Lane', locality: 'Brooklyn', region: 'ny', postal_code: '11201', country_code: 'US' },
    website: 'https://example.com/noodle-house', confidence: 0.97, operating_status: 'open',
  },
  {
    place_id: 'overture:44017831-5500-4780-a4e7-a8fac208e6fe',
    name: 'Corner Café', lat: 40.68884, lon: -73.97991, distance_mi: 1.2,
    category: 'cafe', categories: ['cafe'],
    address: { street: '3 Example Square', locality: 'Brooklyn', region: 'ny', postal_code: '11217', country_code: 'US' },
    website: 'http://example.com/corner-cafe', confidence: 0.9, operating_status: 'open',
  },
  {
    place_id: 'overture:9b95db77-1739-4ab5-b427-7692069ca574',
    name: 'Night Market', lat: 40.691357, lon: -73.982471, distance_mi: 2.6,
    category: 'restaurant', categories: ['restaurant'],
    address: { locality: 'Brooklyn', country_code: 'US' },
    confidence: 0.72,
  },
]
const places = http.createServer((req, res) => {
  const url = new URL(req.url, `http://localhost:${PLACES_PORT}`)
  const reply = (status, body) => {
    res.writeHead(status, { 'content-type': 'application/json', 'x-request-id': 'dev', 'cache-control': 'no-store' })
    res.end(JSON.stringify(body))
  }
  if (req.method !== 'GET' || url.pathname !== '/v1/places') {
    return reply(404, { error: { code: 'not_found', message: 'No such endpoint.' } })
  }
  if (req.headers.authorization !== 'Bearer dev-key') {
    return reply(401, { error: { code: 'unauthenticated', message: 'Missing bearer API key.' } })
  }
  const q = (url.searchParams.get('q') ?? '').toLowerCase()
  if (!url.searchParams.get('lat') || !url.searchParams.get('lon')) {
    return reply(400, { error: { code: 'validation_error', message: 'lat and lon are required.' } })
  }
  if (q.includes('quota')) {
    return reply(402, { error: { code: 'quota_exhausted', message: 'Monthly quota exhausted.' } })
  }
  const results = q.includes('nothing') ? [] : PLACES
  reply(200, { results, meta: { request_id: 'dev', data_source: 'overture', data_release: '2026-08-19.0', q, warnings: [] } })
})
await new Promise((resolve) => places.listen(PLACES_PORT, '127.0.0.1', resolve))

const envDev = [
  `# Written by scripts/dev-env.mjs at ${new Date().toISOString()}. Regenerated on every start.`,
  'EATEN_AT_DEV_INSECURE=1',
  `EATEN_AT_PLC_DIRECTORY=http://localhost:${PLC_PORT}`,
  `EATEN_AT_DEV_HOSTS=eaten.test=127.0.0.1:${PDS_PORT},alice.test=127.0.0.1:${PDS_PORT}`,
  // The lexicon DNS name comes from the NSID authority (at.eaten →
  // eaten.at), not from the publisher's handle.
  `EATEN_AT_DEV_DNS_TXT=_lexicon.eaten.at=did=${publisher.did}`,
  `EATEN_AT_LEXICON_IDENTIFIER=eaten.test`,
  `EATEN_AT_LEXICON_APP_PASSWORD=${publisher.appPassword}`,
  `EATEN_AT_DEV_PDS=http://localhost:${PDS_PORT}`,
  `EATEN_AT_DEV_ALICE_DID=${alice.did}`,
  `EATEN_AT_DEV_ALICE_THEMED_PUBLICATION=${themedPublicationUri.split('/').pop()}`,
  `EATEN_AT_DB=${DEV_CACHE}`,
  // The stub above stands in for Open Places; the real key in .env is
  // never used against the local network.
  `EATEN_AT_PLACES_API_URL=http://localhost:${PLACES_PORT}`,
  'EATEN_AT_PLACES_API_KEY=dev-key',
  '',
].join('\n')
await writeFile(path.join(ROOT, '.env.dev'), envDev)

console.log(`
Local atproto network is up (in memory; Ctrl-C to stop).

  PLC   http://localhost:${PLC_PORT}
  PDS   http://localhost:${PDS_PORT}
  Open Places stub  http://localhost:${PLACES_PORT}   (any search finds three places; "nothing" finds none; "quota" is refused)

  eaten.test  ${publisher.did}   lexicon publisher (app password in .env.dev)
  alice.test  ${alice.did}   author, password "${PASSWORD}"
              publication ${publicationUri}   Field Notes (site palette, the default)
              publication ${themedPublicationUri}   After Hours (author theme)
              documents   ${docUris.join('\n              ')}

Wrote .env.dev and cleared ${DEV_CACHE}. In another terminal:

  just run-dev                                 # the app, at http://127.0.0.1:3000/@alice.test
  just lexicons-check-dev                      # dry run against eaten.test
  just lexicons-publish-dev                    # publish and verify via _lexicon.eaten.at (overridden)
`)

const shutdown = async () => {
  console.log('\nStopping dev network…')
  places.close()
  await network.close()
  process.exit(0)
}
process.on('SIGINT', shutdown)
process.on('SIGTERM', shutdown)
await new Promise(() => {})
