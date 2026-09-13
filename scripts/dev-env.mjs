#!/usr/bin/env node
// Start a local atproto network (PLC + PDS, no AppView, no Docker) from a
// sibling checkout of bluesky-social/atproto, seed it with a lexicon
// publisher account and an author with a few write-ups, and write the
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

const { uri: publicationUri } = await createRecord(alice.agent, 'site.standard.publication', {
  $type: 'site.standard.publication',
  url: 'https://alice.eaten.test',
  name: 'Field Notes',
  description: 'Write-ups by Alice, one subject at a time.',
  // Each colour is a union member and the PDS validates it, so $type is required.
  basicTheme: {
    $type: 'site.standard.theme.basic',
    background: rgb(250, 248, 240),
    foreground: rgb(200, 200, 190), // fails contrast on purpose
    accent: rgb(0, 132, 180),
    accentForeground: rgb(255, 255, 255),
  },
})

// A Bluesky post to hang a comment thread on. The PDS validates the
// strong ref's CID, so it has to be a real record.
const { uri: postUri, cid: postCid } = await createRecord(alice.agent, 'app.bsky.feed.post', {
  $type: 'app.bsky.feed.post',
  text: 'New write-up: First Subject.',
  createdAt: '2026-07-28T13:00:00.000Z',
})

// Plain text for `textContent`: markdown syntax stripped, one paragraph
// per line. Good enough for a seed; the app derives its own excerpts.
const plain = (md) =>
  md
    .replace(/^#+\s+/gm, '')
    .replace(/^>\s?/gm, '')
    .replace(/^[-*]\s+/gm, '')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/[*_`]/g, '')
    .replace(/\n{2,}/g, '\n')
    .trim()

// Placeholder write-ups. Enough to see a listing, a document with a
// description and one whose excerpt is derived, a comment section, tags,
// and links out; the prose is invented for the seed.
const docs = [
  {
    title: 'A first write-up',
    path: '/2026/09/first-subject',
    publishedAt: '2026-09-07T12:00:00.000Z',
    tags: ['notes', 'one long sit'],
    description: 'What a write-up with a subject, an excerpt, tags, and a comment thread looks like.',
    subject: 'First Subject',
    urls: [
      { url: 'https://example.com/first', service: 'officialSite' },
      { url: 'https://example.com/first/elsewhere', service: 'shop', label: 'Elsewhere' },
    ],
    bskyPostRef: { uri: postUri, cid: postCid },
    markdown: `# A first write-up

This is the newest document in the seed, so it leads the listing. It has an
author-written *description*, which the listing and the link previews use
as the excerpt.

## What to look for

- The subject card above this prose: the subject's title beside a
  generated placeholder cover.
- Two links in the footer: one labelled from its known service, one by
  the label the author gave it.
- A comment section, because the record names a Bluesky post. The local
  network has no AppView, so it stays empty here.`,
  },
  {
    title: 'A second write-up',
    path: '/2026/08/second-subject',
    publishedAt: '2026-08-15T09:30:00.000Z',
    tags: ['notes'],
    subject: 'Second Subject',
    urls: [{ url: 'https://example.com/second', service: 'officialSite' }],
    markdown: `This one has no description, so the listing derives its excerpt from the
first paragraph of the body, cut at a sentence boundary.

> A blockquote, to show the hairline rule the prose uses for one.

And a second paragraph that the excerpt never reaches.`,
  },
  {
    title: 'A third write-up',
    path: '/2026/07/third-subject',
    publishedAt: '2026-07-28T13:00:00.000Z',
    tags: ['long read'],
    subject: 'Third Subject',
    urls: [],
    markdown: `The oldest of the three, with no links out at all: the footer shows only
the tags, the feed, and the author.

Tags are the author's own words. \`long read\` is one here, matched
loosely on the tag page, so \`Long  Read\` finds it too.`,
  },
]
// Created oldest first so record keys (TIDs) run in publish order, as they
// would for a real author; listings page through keys, newest first.
const docUris = []
for (const d of [...docs].reverse()) {
  const { uri } = await createRecord(alice.agent, 'site.standard.document', {
    $type: 'site.standard.document',
    site: publicationUri,
    title: d.title,
    path: d.path,
    publishedAt: d.publishedAt,
    tags: d.tags,
    ...(d.description ? { description: d.description } : {}),
    ...(d.bskyPostRef ? { bskyPostRef: d.bskyPostRef } : {}),
    content: {
      $type: 'at.markpub.markdown',
      flavor: 'commonmark',
      text: { $type: 'at.markpub.text', markdown: d.markdown },
    },
    textContent: plain(d.markdown),
    links: {
      $type: 'at.eaten.subject',
      title: d.subject,
      externalUrls: d.urls,
    },
  })
  docUris.push(uri)
}
// A document without a subject in the same publication, to prove filtering.
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
              publication ${publicationUri}
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
