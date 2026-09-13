#!/usr/bin/env node
// Start a local atproto network (PLC + PDS, no AppView, no Docker) from a
// sibling checkout of bluesky-social/atproto, seed it with a lexicon
// publisher account and an author with a few visits, and write the
// matching EATEN_AT_* variables to .env.dev.
//
//   ATPROTO_DIR=../atproto node scripts/dev-env.mjs
//
// PLC_PORT and PDS_PORT override the ports (default 2582 and 2583), so a
// second local network can run beside another project's.
//
// The network is in memory: everything is recreated on each start. Stop
// with Ctrl-C.

import { rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const ROOT = path.resolve(HERE, '..')
const ATPROTO_DIR = path.resolve(ROOT, process.env.ATPROTO_DIR ?? '../atproto')
const PLC_PORT = Number(process.env.PLC_PORT ?? 2582)
const PDS_PORT = Number(process.env.PDS_PORT ?? 2583)
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
// a comment section, tags, ids, and links out; the prose is invented for
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
      ids: [{ service: 'googlePlace', id: 'ChIJexampleNoodleHouse' }],
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

- The visit card above this prose: the place's name beside a generated
  placeholder cover, then the date, the meal, the price band, the address,
  and the rating as plus signs.
- Links in the footer: two labelled from their known service, one by the
  label the author gave it, and a map link made from the Google place id.
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
      ids: [{ service: 'applePlace', id: 'I1234567890' }],
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
      ids: [{ service: 'overtureGers', id: '08f2a5b6c7d8e9f0a1b2c3d4e5f60718' }],
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
  '',
].join('\n')
await writeFile(path.join(ROOT, '.env.dev'), envDev)

console.log(`
Local atproto network is up (in memory; Ctrl-C to stop).

  PLC   http://localhost:${PLC_PORT}
  PDS   http://localhost:${PDS_PORT}

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
  await network.close()
  process.exit(0)
}
process.on('SIGINT', shutdown)
process.on('SIGTERM', shutdown)
await new Promise(() => {})
