#!/usr/bin/env node
// Render every page of the app, signed out and signed in, in headless
// Chrome at a desktop and a phone width, against the local atproto network
// that `just dev-env` runs. Fails when a page:
//
//   - answers with an unexpected status,
//   - logs a JavaScript error, an exception, or a CSP violation,
//   - loads a subresource that fails (other than /favicon.ico),
//   - fails to load one of the web fonts (Newsreader and JetBrains Mono
//     everywhere; Evantic wherever the logotype is on the page), or
//   - scrolls horizontally.
//
// Full-page screenshots of every page land in target/visual-check/.
//
//   just visual-check            # loads .env.dev, then runs this
//
// The app under test is built from the working tree and started on its own
// port (VISUAL_CHECK_PORT, default 3100), so a `just run-dev` on :3000 is
// left alone. Signing in goes through `dev-session`, which writes a browser
// session straight into the dev database: no password, no OAuth screens.
// Chrome is found at CHROME, or in the usual install locations.

import { spawn, spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const ROOT = path.resolve(HERE, '..')
const OUT = path.join(ROOT, 'target', 'visual-check')
const PORT = Number(process.env.VISUAL_CHECK_PORT ?? 3100)
const BASE = `http://127.0.0.1:${PORT}`
const WIDTHS = [
  { label: 'desktop', width: 1080, height: 900, mobile: false },
  { label: 'phone', width: 390, height: 844, mobile: true },
]
const FONT_FAMILIES = ['Newsreader', 'JetBrains Mono']
// The logotype face is only asked for where the logotype is; a
// publication's inner pages carry a running head in the serif instead.
const LOGOTYPE_FAMILY = 'Evantic'
const LOGOTYPE_SELECTOR = '.logotype'
const NAVIGATION_TIMEOUT_MS = 20_000

const children = []
let tempProfile

main()
  .then((failed) => cleanup().then(() => process.exit(failed ? 1 : 0)))
  .catch(async (err) => {
    console.error(`visual-check: ${err.message}`)
    await cleanup()
    process.exit(2)
  })

async function main() {
  const did = requireEnv('EATEN_AT_DEV_ALICE_DID')
  const bobDid = requireEnv('EATEN_AT_DEV_BOB_DID')
  const themedRkey = requireEnv('EATEN_AT_DEV_ALICE_THEMED_PUBLICATION')
  const pds = requireEnv('EATEN_AT_DEV_PDS')
  if (!process.env.EATEN_AT_DEV_INSECURE) {
    throw new Error('the dev environment is not loaded; run this through `just visual-check`')
  }
  await expectOk(`${pds}/xrpc/_health`, 'the local PDS is not answering; start it with `just dev-env`')

  run('cargo', ['build', '-q', '-p', 'eaten-at', '--bin', 'eaten-at', '--bin', 'dev-session'])
  const appEnv = {
    ...process.env,
    EATEN_AT_LISTEN: `127.0.0.1:${PORT}`,
    EATEN_AT_PUBLIC_URL: BASE,
    RUST_LOG: process.env.RUST_LOG ?? 'warn',
  }
  const app = start(path.join(ROOT, 'target', 'debug', 'eaten-at'), [], appEnv, 'app')
  await waitFor(`${BASE}/healthz`, app, 'the app did not start')

  const cookie = run(path.join(ROOT, 'target', 'debug', 'dev-session'), [did], appEnv).trim()
  const [cookieName, cookieValue] = splitOnce(cookie, '=')
  const bobCookie = run(path.join(ROOT, 'target', 'debug', 'dev-session'), [bobDid], appEnv).trim()
  const [, bobCookieValue] = splitOnce(bobCookie, '=')

  const browser = await Browser.launch()
  await rm(OUT, { recursive: true, force: true })
  await mkdir(OUT, { recursive: true })

  const failures = []
  const check = async (page) => {
    if (process.env.VISUAL_CHECK_FILTER && !page.name.includes(process.env.VISUAL_CHECK_FILTER)) return
    for (const viewport of page.viewports ?? WIDTHS) {
      const problems = await browser.visit(page, viewport)
      const name = `${page.name} @ ${viewport.label}`
      if (problems.length) {
        failures.push({ name, problems })
        console.log(`FAIL  ${name}`)
        for (const p of problems) console.log(`        ${p}`)
      } else {
        console.log(`ok    ${name}`)
      }
    }
  }

  // Signed out. The account's page redirects to its default publication,
  // which has no theme; the second publication has one. Documents and a tag
  // page are found on each front page, so the check follows whatever the
  // seed contains.
  const account = `/at/${did}/`
  const discovered = await browser.discover(account)
  const front = discovered.path
  if (discovered.documents.length === 0) throw new Error(`no documents listed on ${front}`)
  const themedFront = `/at/${did}/${themedRkey}/`
  const themed = await browser.discover(themedFront)
  if (themed.documents.length === 0) throw new Error(`no documents listed on ${themedFront}`)

  await check({ name: 'landing', path: '/', expect: 'a.button[href="/login"]' })
  // Connect (plan 09): pressing the link swaps the sign-in form in and
  // focuses its field, which then suggests handles like the sign-in page.
  await check({
    name: 'landing-connect',
    path: '/',
    steps: [
      { press: '.connect:not(.connect-read) .connect-button', wait: '#connect-handle:focus' },
      { type: { '#connect-handle': 'ali' }, wait: '#connect-handle-list [role="option"]' },
    ],
    expect: '.connect:not(.connect-read) .connect-idle[hidden] + form.connect-form:not([hidden])',
  })
  // The second way in, drawn the same: its button becomes the field for
  // someone else's handle, which suggests handles too.
  await check({
    name: 'landing-lookup',
    path: '/',
    steps: [
      { press: '.connect-read .connect-button', wait: '#lookup-handle:focus' },
      { type: { '#lookup-handle': 'ali' }, wait: '#lookup-handle-list [role="option"]' },
    ],
    expect: '.connect-read .connect-idle[hidden] + form.connect-form:not([hidden])',
  })
  await check({ name: 'about', path: '/about', expect: '.prose a[href="https://db-ip.com"]' })
  await check({ name: 'lookup', path: '/lookup', expect: 'input#handle' })
  await check({ name: 'lookup-error', path: '/lookup?handle=nobody.invalid', status: 400 })
  await check({ name: 'login', path: '/login', expect: 'input[data-typeahead]' })
  // The handle island (plan 10): typing shows suggestions from the stub
  // AppView; the page's policy allows that one origin.
  await check({
    name: 'login-typeahead',
    path: '/login',
    steps: [{ type: { '#handle': 'ali' }, wait: '[role="option"]' }],
    expect: '[role="listbox"] [role="option"]',
  })
  await check({
    name: 'lookup-typeahead',
    path: '/lookup',
    steps: [{ type: { '#handle': 'ali' }, wait: '[role="option"]' }],
    expect: '[role="listbox"] [role="option"]',
  })
  await check({ name: 'handle-redirect', path: '/@alice.test', finalPath: front })
  await check({ name: 'account', path: account, finalPath: front })
  const publicationPages = async (prefix, pub, frontPath, themeSelector) => {
    await check({ name: `${prefix}front`, path: frontPath, expect: themeSelector })
    for (const [i, doc] of pub.documents.entries()) {
      await check({ name: `${prefix}document-${i + 1}`, path: doc, expect: themeSelector })
    }
    if (pub.tag) await check({ name: `${prefix}tag`, path: pub.tag, expect: themeSelector })
  }
  // The site palette must not be overridden where no theme is declared, and
  // an author theme must still apply where one is.
  await publicationPages('', discovered, front, 'html:not([data-theme])')
  await publicationPages('themed-', themed, themedFront, 'html[data-theme="publication"]')
  await check({ name: 'not-found', path: `/at/${did}/nope/nope`, status: 404 })
  await check({ name: 'write-signed-out', path: '/write', finalPath: '/login' })

  // Signed in.
  await browser.setCookie(cookieName, cookieValue)
  for (const outcome of ['published', 'saved']) {
    await check({
      name: `post-publication-${outcome}`,
      path: `${discovered.documents[0]}?after=${outcome}`,
      expect: '.publish-confirmation',
      exercise: async browser => {
        const valid = await browser.evaluate(`(() => {
          const panel = document.querySelector('.publish-confirmation');
          const canonical = document.querySelector('link[rel="canonical"]').href;
          const share = new URL(panel.querySelector('a[target="_blank"]').href);
          return !new URL(location.href).searchParams.has('after') &&
            share.searchParams.get('text') === canonical &&
            panel.querySelector('.permalink').href === canonical;
        })()`);
        if (!valid) throw new Error('Confirmation retained its query or shared the wrong permalink');
      },
    });
  }
  await check({
    name: 'post-publication-copy',
    path: `${discovered.documents[0]}?after=published`,
    expect: '.publish-confirmation',
    exercise: async browser => {
      await browser.session.send('Browser.grantPermissions', { origin: BASE, permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] });
      await browser.evaluate(`document.querySelector('.copy-permalink').click()`);
      if (!await browser.appears('.copy-status:not(:empty)')) throw new Error('No copy feedback');
      if (!await browser.evaluate(`(async () => document.querySelector('.copy-status').textContent === 'Link copied.' && await navigator.clipboard.readText() === document.querySelector('link[rel="canonical"]').href)()`)) throw new Error('Copy did not put the canonical permalink on the clipboard');
      // A denied clipboard must expose the plain link again.
      await browser.evaluate(`(() => {
        navigator.clipboard.writeText = async () => { throw new Error('denied'); };
        document.querySelector('.copy-status').textContent = '';
        document.querySelector('.copy-permalink').click();
      })()`);
      if (!await browser.appears('.permalink:not([hidden])')) throw new Error('No clipboard fallback');
    },
  });
  await check({ name: 'publishing-reconnected', path: '/login/reconnected', expect: '.page-head .lede' });
  // Discover photo and empty-photo states; individual exercises supply
  // their own prose and teaser content without saving records.
  if (discovered.withPhotos.length === 0) throw new Error(`no write-up with photos listed on ${front}`)
  if (discovered.withoutPhotos.length === 0) throw new Error(`no write-up without photos listed on ${front}`)
  const rkey = discovered.withPhotos[0].split('/').pop()
  const bare = discovered.withoutPhotos[discovered.withoutPhotos.length - 1].split('/').pop()
  await check({ name: 'landing-signed-in', path: '/', expect: '.own-publication .listing-item' })
  await check({ name: 'landing-find', path: '/?q=noodle', expect: '.own-publication .listing-item' })
  await check({ name: 'landing-find-none', path: '/?q=zzz', expect: '.own-publication .empty' })
  // The live find (plan 11): typing swaps the results in without a reload.
  await check({
    name: 'landing-find-live',
    path: '/',
    steps: [{ type: { '#q': 'noodle' }, wait: '.find-results a.button-link[href="/"]' }],
    expect: '.find-results .listing-item',
  })
  await check({ name: 'settings', path: '/settings', expect: '.chooser-item' })
  // The editor starts by choosing a place (plans 06, 12; the Write Pages
  // handoff): the place's name as the headline, suggesting as it is
  // typed, the address under it, and Start writing. The request is
  // located by EATEN_AT_DEV_LOCATION, so suggestions are on.
  const byHand = (name) => ({ fill: { '#place_name': name }, click: '#start-writing' })
  await check({ name: 'write', path: '/write', expect: '#place_name[data-suggest]' })
  await check({
    name: 'write-suggest',
    path: '/write',
    steps: [{ type: { '#place_name': 'noo' }, wait: '[role="option"]' }],
    expect: '[role="listbox"] [role="option"]',
  })
  // A pick fills both lines and arms Start writing as the pick; pressing
  // it lands in the editor with the listing behind the place.
  await check({
    name: 'write-picked',
    path: '/write',
    steps: [
      { type: { '#place_name': 'noo' }, wait: '[role="option"]' },
      { press: '[role="option"]', wait: '#start-writing[value^="pick:"]' },
    ],
    expect: '#start-writing[value^="pick:"]:not([hidden])',
  })
  await check({
    name: 'write-pick',
    path: '/write',
    steps: [
      { type: { '#place_name': 'noo' }, wait: '[role="option"]' },
      { press: '[role="option"]', wait: '#start-writing[value^="pick:"]' },
      { click: '#start-writing' },
    ],
    expect: 'input[name="place_mode"][value="picked"]',
  })
  await check({
    name: 'write-manual',
    path: '/write',
    steps: [byHand('The Cart')],
    expect: '.digest-editor.digest-empty .digest-ghost em',
  })
  await check({
    name: 'write-manual-blank',
    path: '/write',
    steps: [{ click: '#start-writing' }],
    status: 422,
    expect: '#place_name-error',
  })
  await check({
    name: 'write-errors',
    path: '/write',
    steps: [byHand('The Cart'), { click: 'form.editor button[name="action"][value="publish"]' }],
    status: 422,
    expect: '.field-error',
  })
  // The editing screen and its islands: the live markdown editor, the
  // calendar, the meal menu, the tags as chips, the teaser fold, a link
  // card, the photos in place, and Delete's confirmation.
  await check({ name: 'edit', path: `/write/${rkey}`, expect: '.digest-editor .md-line' })
  await check({
    name: 'edit-short-draft', path: `/write/${rkey}`,
    viewports: [{ label: 'laptop', width: 1440, height: 800, mobile: false }],
    expect: '.photo-tile',
    exercise: async (browser) => {
      await browser.evaluate(`(() => {
        const body = document.querySelector('#body')
        body.value = 'The noodles had a little bite. We stayed for another pot of tea.'
        body.dispatchEvent(new Event('change', { bubbles: true }))
        const teaser = document.querySelector('#description')
        teaser.value = ''
        teaser.dispatchEvent(new Event('input', { bubbles: true }))
        document.querySelector('details.teaser').open = false
        document.activeElement.blur()
        scrollTo(0, 0)
      })()`)
      const lines = await browser.evaluate(`(() => { const box = document.querySelector('.digest-editor'); return box.getBoundingClientRect().height / parseFloat(getComputedStyle(box).lineHeight) })()`)
      if (Math.abs(lines - 6) > 0.1) throw new Error('Short digest should start at six lines: ' + lines)
    },
  })
  await check({ name: 'edit-selection', path: `/write/${rkey}`, exercise: checkDigestSelection, expect: '.digest-editor .md-line' })
  await check({ name: 'edit-restore-place', path: `/write/${rkey}`, exercise: checkPlaceDraft, expect: '#change_restaurant' })
  await check({
    name: 'edit-long-tags-links', path: `/write/${rkey}`,
    steps: [{ type: { '#tags': 'a very long tag about dinner, another long tag about the restaurant, a third tag,' }, wait: '.tag-field .chip' }],
    expect: '.photos-block + .tags-line',
  })
  await check({
    name: 'edit-digest',
    path: `/write/${rkey}`,
    exercise: async (browser) => browser.evaluate(`(() => {
      const body = document.querySelector('#body')
      body.value = '# A heading'
      body.dispatchEvent(new Event('change', { bubbles: true }))
      document.querySelector('.md-line').click()
    })()`),
    expect: '.md-active .md-tok',
  })
  await check({
    name: 'edit-date',
    path: `/write/${rkey}`,
    steps: [{ press: '.date-button', wait: '.date-popover:not([hidden]) .date-day.selected' }],
    expect: '#visited_on[hidden]',
  })
  await check({
    name: 'edit-meal',
    path: `/write/${rkey}`,
    steps: [{ press: '[data-note="meal"] .note-button', wait: '.note-popover:not([hidden]) .note-option' }],
    expect: '.date-line [data-note="meal"] .note-clear',
  })
  await check({
    name: 'edit-tags',
    path: `/write/${rkey}`,
    steps: [{ type: { '#tags': 'late night,' }, wait: '.tag-field .chip[title="Remove late night"]' }],
    expect: '#tags-value[name="tags"]',
  })
  // The write-up without a teaser of its own folds the line; the one
  // with one opens on it.
  await check({
    name: 'edit-teaser',
    path: `/write/${bare}`,
    exercise: async (browser) => browser.evaluate(`(() => {
      const description = document.querySelector('#description')
      description.value = ''
      description.dispatchEvent(new Event('input', { bubbles: true }))
      document.querySelector('details.teaser').open = false
      document.querySelector('details.teaser > summary').click()
    })()`),
    expect: '.teaser-default:not([hidden])',
  })
  await check({
    name: 'edit-teaser-custom', path: `/write/${rkey}`,
    steps: [{ type: { '#description': 'A short invitation to read.' }, wait: 'details.teaser[open]' }],
    expect: 'details.teaser[open] .teaser-custom:not([hidden])',
  })
  // Without photos, the box is the invitation.
  await check({
    name: 'edit-publish-draft-safety', path: `/write/${bare}`,
    expect: 'form.editor',
    exercise: async browser => {
      const retained = await browser.evaluate(`(() => {
        const form = document.querySelector('form.editor');
        form.elements.body.value = 'A BLT worth remembering.';
        form.elements.body.dispatchEvent(new Event('input', { bubbles: true }));
        // Exercise submit listeners without writing a record to the local PDS.
        form.dispatchEvent(new SubmitEvent('submit', { cancelable: true, submitter: form.querySelector('[value="publish"]') }));
        return JSON.parse(localStorage.getItem('ea:draft:' + location.pathname)).data.body;
      })()`);
      if (retained !== 'A BLT worth remembering.') throw new Error('Publishing discarded the draft before confirmation');
      const draftPath = await browser.evaluate('location.pathname');
      const draftId = await browser.evaluate(`document.querySelector('[name="draft_id"]').value`);
      await browser.navigate(`${BASE}${discovered.withoutPhotos[discovered.withoutPhotos.length - 1]}?after=saved&draft=1`);
      if (!await browser.evaluate(`localStorage.getItem('ea:draft:' + ${JSON.stringify(draftPath)}) !== null`)) throw new Error('An old confirmation discarded a newer draft');
      await browser.navigate(`${BASE}${discovered.withoutPhotos[discovered.withoutPhotos.length - 1]}?after=saved&draft=${draftId}`);
      if (!await browser.evaluate(`localStorage.getItem('ea:draft:' + ${JSON.stringify(draftPath)}) === null`)) throw new Error('Successful save retained its draft');
      await browser.navigate(`${BASE}${draftPath}`);
    },
  });
  await check({ name: 'edit-no-photos', path: `/write/${bare}`, expect: 'button.photo-empty' })
  await check({
    name: 'edit-link',
    path: `/write/${rkey}`,
    steps: [{ press: '.link-word', wait: '.link-card:not([hidden]) .link-card-keep:not([hidden])' }],
    expect: '.link-card:not([hidden]) input[type="url"]',
  })
  await check({
    name: 'edit-photo-detail',
    exercise: checkPhotoDialog,
    path: `/write/${rkey}`,
    steps: [{ press: '.photo-tile', wait: '.photo-scrim .photo-caption' }],
    expect: '.photo-detail img',
  })
  await check({
    name: 'edit-photo-remove', path: `/write/${rkey}`,
    expect: 'button.photo-empty',
    exercise: async (browser) => {
      const ok = await browser.evaluate(`(() => {
        while (document.querySelector('.photo-tile')) {
          document.querySelector('.photo-tile').click()
          document.querySelector('.photo-detail .danger').click()
          if (document.querySelector('.photo-scrim') || !document.activeElement.matches('.photo-tile, .photo-add, .photo-empty')) return false
        }
        return document.activeElement.matches('.photo-empty') && document.querySelector('.photo-fields').children.length === 0
      })()`)
      if (!ok) throw new Error('Removing photos lost focus or retained removed photo fields')
    },
  })
  await check({
    name: 'edit-photo-detail-roomy',
    exercise: checkPhotoDialog,
    path: `/write/${rkey}`,
    viewports: [
      { label: 'wide', width: 1600, height: 1100, mobile: false },
      { label: 'short', width: 1080, height: 540, mobile: false },
    ],
    steps: [{ press: '.photo-tile', wait: '.photo-scrim .photo-caption' }],
    expect: '.photo-detail img',
  })
  await check({
    name: 'edit-delete',
    path: `/write/${rkey}`,
    steps: [{ press: 'details.delete-confirm > summary', wait: 'details.delete-confirm[open] .confirm-yes' }],
    expect: '.confirm-yes[formaction$="/delete"]',
  })
  await check({
    name: 'edit-change-place',
    path: `/write/${rkey}`,
    steps: [{ click: 'button[name="action"][value="change_place"]' }],
    expect: '#place_name.headline[value]',
    exercise: async (browser) => {
      const selected = await browser.evaluate(`(() => {
        const name = document.querySelector('#place_name')
        return name.value.length > 0 && document.activeElement === name && name.selectionStart === 0 && name.selectionEnd === name.value.length
      })()`)
      if (!selected) throw new Error('The current restaurant name was not fully selected')
      if (!await browser.evaluate(`document.querySelector('#start-writing').textContent.trim() === 'Keep writing'`)) throw new Error('Reselecting a restaurant should offer Keep writing')
      await browser.session.send('Input.insertText', { text: 'St. John Bread and Wine' })
      if (!await browser.evaluate(`document.querySelector('#place_name').value === 'St. John Bread and Wine'`)) throw new Error('Typing did not replace the restaurant name')
    },
  })
  await check({
    name: 'edit-keep',
    path: `/write/${rkey}`,
    submit: 'form.editor button[name="action"][value="keep"]',
    expect: 'form.editor-write',
  })
  await check({ name: 'photos', path: `/write/${rkey}/photos`, expect: '.photo-row' })
  await check({ name: 'photos-empty', path: `/write/${bare}/photos`, expect: '.empty' })
  await check({
    name: 'photos-new',
    path: `/write/${bare}/photos?new=1&then=${encodeURIComponent(discovered.documents[0])}`,
    expect: 'a.button-link[href="' + discovered.documents[0] + '"]',
  })
  await check({ name: 'delete', path: `/write/${rkey}/delete` })
  await check({ name: 'crosspost', path: `/write/${rkey}/crosspost` })

  // An author with no publication yet (plan 08): settings offers to make
  // it, and the editor does not ask.
  await browser.setCookie(cookieName, bobCookieValue)
  await check({ name: 'landing-no-publication', path: '/', expect: '.own-none' })
  await check({ name: 'settings-none', path: '/settings', expect: '.chooser-item.not-yet' })
  await check({ name: 'write-none', path: '/write', expect: '#place_name.headline' })

  await browser.close()
  const total = failures.reduce((n, f) => n + f.problems.length, 0)
  console.log(
    failures.length
      ? `\n${failures.length} page view(s) with ${total} problem(s). Screenshots: ${path.relative(ROOT, OUT)}/`
      : `\nAll pages passed. Screenshots: ${path.relative(ROOT, OUT)}/`,
  )
  return failures.length > 0
}

// Exercise real keyboard selection and replacement in the browser. Clipboard
// events use an in-memory clipboard so these checks never touch the user's.
async function checkDigestSelection(browser) {
  const original = '# Heading\n\nA **bold** line.\n\nThe last *line*.'
  const expectBody = async (value) => {
    const actual = await browser.evaluate('document.querySelector("#body").value')
    if (actual !== value) throw new Error(`digest mismatch: ${JSON.stringify(actual)}`)
  }
  const key = async (key, modifiers = 2) => {
    const code = 'Key' + key.toUpperCase()
    await browser.session.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, modifiers, windowsVirtualKeyCode: key.toUpperCase().charCodeAt(0) })
    await browser.session.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, modifiers })
  }
  const selectAll = async (modifiers = 2) => {
    await key('a', modifiers)
    const facts = await browser.evaluate(`(() => {
      const box = document.querySelector('.digest-editor')
      const sel = getSelection()
      return { focus: document.activeElement.outerHTML.slice(0,180), anchor: sel.anchorNode?.nodeName, selected: sel.toString(), active: box.querySelectorAll('.md-active').length,
        folded: [...box.querySelectorAll('.md-line:not(.md-active)')].every(n => !n.querySelector('.md-tok')) }
    })()`)
    if (!facts.selected.includes('Heading') || !facts.selected.includes('last') || !facts.folded || facts.active !== 1) {
      throw new Error('Select All must span the digest and keep inactive lines folded: ' + JSON.stringify({modifiers, ...facts}))
    }
  }
  const clipboard = async (type, text = '') => browser.evaluate(`(() => {
    const data = new DataTransfer()
    data.setData('text/plain', ${JSON.stringify(text)})
    document.querySelector('.md-active').dispatchEvent(new ClipboardEvent(${JSON.stringify(type)}, { bubbles: true, cancelable: true, clipboardData: data }))
    return data.getData('text/plain')
  })()`)
  await browser.evaluate(`(() => {
    const ta = document.querySelector('#body'); ta.value = ${JSON.stringify(original)}
    ta.dispatchEvent(new Event('change', { bubbles: true }))
    document.querySelectorAll('.md-line')[2].click()
  })()`)
  await selectAll()
  if (await clipboard('copy') !== original) throw new Error('Copy lost markdown or line breaks')
  await expectBody(original)
  if (await clipboard('cut') !== original) throw new Error('Cut lost markdown or line breaks')
  await expectBody('')
  await key('z')
  await expectBody(original)
  await selectAll(4) // Cmd+A as well as Ctrl+A.
  await browser.session.send('Input.insertText', { text: 'Replacement' })
  await expectBody('Replacement')
  await key('z')
  await expectBody(original)
  await key('z', 10) // Ctrl+Shift+Z
  await expectBody('Replacement')
  await key('z')
  await selectAll()
  await clipboard('paste', 'Pasted **one**\n\nAnd two')
  await expectBody('Pasted **one**\n\nAnd two')
  await key('z')
  await expectBody(original)
  for (const [navigation, vk, atStart, modifiers = 0] of [['ArrowLeft', 37, true], ['ArrowRight', 39, false], ['ArrowUp', 38, true], ['ArrowDown', 40, false], ['Home', 36, true], ['End', 35, false], ['ArrowRight', 39, false, 2], ['ArrowRight', 39, false, 4], ['ArrowLeft', 37, true, 1]]) {
    await selectAll()
    await browser.session.send('Input.dispatchKeyEvent', { type: 'keyDown', key: navigation, code: navigation, windowsVirtualKeyCode: vk, modifiers })
    await browser.session.send('Input.dispatchKeyEvent', { type: 'keyUp', key: navigation, code: navigation, windowsVirtualKeyCode: vk, modifiers })
    if (!await browser.evaluate('getSelection().isCollapsed')) throw new Error(navigation + ' did not collapse the digest selection')
    await browser.session.send('Input.insertText', { text: '!' })
    await expectBody(atStart ? '!' + original : original + '!')
    await key('z')
    await expectBody(original)
  }
}

async function checkPlaceDraft(browser) {
  const url = await browser.evaluate('location.href')
  const readPlace = () => browser.evaluate(`(() => {
    const form = document.querySelector('form.editor-write')
    return Object.fromEntries(['place_name', 'place_address', 'place_mode', 'gers_id', 'lat_e6', 'lon_e6'].map(name => [name, form.elements[name].value]))
  })()`)
  const original = await readPlace()
  const submit = (selector) => browser.navigate(null, [], () => browser.evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`))
  for (const picked of [true, false]) {
    await submit('#change_restaurant')
    await browser.evaluate(`(() => {
      const name = document.querySelector('#place_name')
      name.value = ${JSON.stringify(picked ? 'noo' : 'Review Bistro')}
      name.dispatchEvent(new Event('input', { bubbles: true }))
      document.querySelector('#place_address').value = ''
    })()`)
    if (picked) {
      if (!await browser.appears('[role="option"]')) throw new Error('No restaurant suggestion for the draft test')
      await browser.evaluate(`document.querySelector('[role="option"]').click()`)
    }
    await submit('#start-writing')
    const expected = await readPlace()
    if (picked !== !!expected.gers_id || (picked && (!expected.lat_e6 || !expected.lon_e6))) throw new Error('Place fixture did not carry the expected identity')
    await browser.evaluate(`(() => {
      const body = document.querySelector('#body')
      body.value = 'A draft for the changed restaurant.'
      body.dispatchEvent(new Event('change', { bubbles: true }))
      body.dispatchEvent(new Event('input', { bubbles: true }))
    })()`)
    await sleep(600)
    await browser.navigate(url, [])
    await browser.evaluate(`document.querySelector('.restore button').click()`)
    if (JSON.stringify(await readPlace()) !== JSON.stringify(expected)) throw new Error('Restoring the draft lost restaurant details or identity')
    const visible = await browser.evaluate(`({ name: document.querySelector('#title').placeholder, title: document.querySelector('#title').value, address: document.querySelector('.place-address-text').textContent, hidden: document.querySelector('.place-where').hidden, body: document.querySelector('#body').value })`)
    // The place line shows "at …" when there is an address, or a title of its own to name the place under.
    if (visible.name !== expected.place_name || visible.address !== expected.place_address.replace(/, [A-Z]{2} [\w-]+$/, (m) => m.slice(0, 4)).replace(/, \d[\w-]*$/, '') || visible.hidden !== !(expected.place_address || visible.title.trim()) || visible.body !== 'A draft for the changed restaurant.') throw new Error('Restored place fields and visible composer disagree: ' + JSON.stringify(visible))
  }
  // Legacy drafts cannot safely restore a name without its place identity.
  await browser.evaluate(`(() => {
    const key = 'ea:draft:' + location.pathname
    const saved = JSON.parse(localStorage.getItem(key))
    delete saved.data.gers_id
    saved.data.place_name = 'An incomplete legacy restaurant'
    localStorage.setItem(key, JSON.stringify(saved))
  })()`)
  await browser.navigate(url, [])
  if (!await browser.evaluate(`document.querySelector('.restore').textContent.includes('Check the restaurant')`)) throw new Error('Legacy restoration should explain the missing restaurant details')
  await browser.evaluate(`document.querySelector('.restore button').click()`)
  if (JSON.stringify(await readPlace()) !== JSON.stringify(original)) throw new Error('A legacy draft mixed incomplete restaurant metadata with the saved record')
}

async function checkPhotoDialog(browser) {
  const layout = await browser.evaluate(`(() => {
    const panel = document.querySelector('.photo-detail').getBoundingClientRect()
    const foot = document.querySelector('.photo-detail-foot').getBoundingClientRect()
    return { width: panel.width, top: panel.top, bottom: panel.bottom, actions: foot.bottom, height: innerHeight, viewport: innerWidth }
  })()`)
  if (layout.top < 0 || layout.bottom > layout.height || layout.actions > layout.height) throw new Error('Photo or caption actions extend below the window')
  if (layout.viewport >= 1200 && layout.width < 900) throw new Error('The photo dialog is not using the available width')
  const key = async (key, code, modifiers = 0) => {
    const event = { key, code, modifiers, windowsVirtualKeyCode: key === 'Tab' ? 9 : 27 }
    await browser.session.send('Input.dispatchKeyEvent', { ...event, type: 'keyDown' })
    await browser.session.send('Input.dispatchKeyEvent', { ...event, type: 'keyUp' })
  }
  await key('Tab', 'Tab', 8)
  if (!await browser.evaluate(`document.activeElement.matches('.photo-detail .danger')`)) throw new Error('Shift+Tab left the photo dialog')
  await key('Tab', 'Tab')
  if (!await browser.evaluate(`document.activeElement.matches('.photo-caption')`)) throw new Error('Tab left the photo dialog')
  await key('Escape', 'Escape')
  if (!await browser.evaluate(`!document.querySelector('.photo-scrim') && document.activeElement.matches('.photo-tile')`)) throw new Error('Closing the photo dialog lost keyboard focus')
  await browser.evaluate(`document.activeElement.click()`)
  await browser.evaluate(`(() => { document.querySelector('.photo-caption').value = 'A caption kept on Done'; document.querySelector('.photo-detail .hint-action').click() })()`)
  if (!await browser.evaluate(`document.querySelector('[name="photo_alt_0"]').value === 'A caption kept on Done'`)) throw new Error('Done discarded the caption')
  await browser.evaluate(`document.activeElement.click()`)
}

// ---- Chrome over the DevTools protocol ----

class Browser {
  static async launch() {
    const binary = findChrome()
    tempProfile = await mkdtemp(path.join(os.tmpdir(), 'eaten-at-visual-check-'))
    start(
      binary,
      [
        '--headless=new',
        '--disable-gpu',
        '--hide-scrollbars',
        '--no-first-run',
        '--no-default-browser-check',
        `--user-data-dir=${tempProfile}`,
        '--remote-debugging-port=0',
        'about:blank',
      ],
      process.env,
      'chrome',
    )
    // Chrome writes the port it picked into the profile directory.
    const portFile = path.join(tempProfile, 'DevToolsActivePort')
    for (let i = 0; i < 100 && !existsSync(portFile); i++) await sleep(100)
    if (!existsSync(portFile)) throw new Error('Chrome did not open a debugging port')
    const [port] = (await readFile(portFile, 'utf8')).split('\n')
    const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json()
    const browser = new Browser(await Session.connect(target.webSocketDebuggerUrl))
    await browser.session.send('Page.enable')
    // A headless page is never the focused window, so :focus would match
    // nothing; act as if it were, so a step can wait on a focused field.
    await browser.session.send('Emulation.setFocusEmulationEnabled', { enabled: true })
    await browser.session.send('Network.enable')
    await browser.session.send('Runtime.enable')
    await browser.session.send('Log.enable')
    return browser
  }

  constructor(session) {
    this.session = session
  }

  async setCookie(name, value) {
    await this.session.send('Network.setCookie', { name, value, url: BASE, httpOnly: true, sameSite: 'Lax' })
  }

  /** Links to every listed document and the first tag, from a front page. */
  async discover(pagePath) {
    await this.session.send('Emulation.setDeviceMetricsOverride', { ...WIDTHS[0], deviceScaleFactor: 1 })
    await this.navigate(`${BASE}${pagePath}`)
    return this.evaluate(`({
      path: location.pathname,
      documents: [...document.querySelectorAll('.listing-title a')].map(a => a.getAttribute('href')),
      withPhotos: [...document.querySelectorAll('.listing-item.has-photos .listing-title a')].map(a => a.getAttribute('href')),
      withoutPhotos: [...document.querySelectorAll('.listing-item.no-photos .listing-title a')].map(a => a.getAttribute('href')),
      tag: document.querySelector('.nameplate .tag')?.getAttribute('href') ?? null,
    })`)
  }

  /** Load one page at one viewport and return what is wrong with it. */
  async visit(page, viewport) {
    const problems = []
    await this.session.send('Emulation.setDeviceMetricsOverride', {
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: 1,
      mobile: viewport.mobile,
    })
    // Every page view starts from a clean draft store, so the editor's
    // restore banner never depends on an earlier visit.
    await this.session.send('Storage.clearDataForOrigin', { origin: BASE, storageTypes: 'local_storage' })

    let status = await this.navigate(`${BASE}${page.path}`, problems)
    // A page may be reached through the form: each step fills fields,
    // then clicks a button that submits, and the last response is the
    // page under test. `submit` is the one-click shorthand.
    const steps = page.steps ?? (page.submit ? [{ click: page.submit }] : [])
    for (const step of steps) {
      // A `press` step clicks something in the page and waits for `wait`
      // to appear; no navigation follows.
      if (step.press) {
        const pressed = await this.evaluate(
          `(() => { const el = document.querySelector(${JSON.stringify(step.press)}); if (!el) return false; el.click(); return true })()`,
        )
        if (!pressed) problems.push(`no element matches ${step.press}`)
        else if (!(await this.appears(step.wait))) problems.push(`nothing matched ${step.wait} after pressing ${step.press}`)
        continue
      }
      // A `type` step types into a field (an input event, so an island
      // reacts) and waits for `wait` to appear; no navigation follows.
      if (step.type) {
        for (const [selector, value] of Object.entries(step.type)) {
          const typed = await this.evaluate(
            `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return false; el.focus(); el.value = ${JSON.stringify(value)}; el.dispatchEvent(new Event('input', { bubbles: true })); return true })()`,
          )
          if (!typed) problems.push(`no element matches ${selector}`)
        }
        if (!(await this.appears(step.wait))) problems.push(`nothing matched ${step.wait} after typing`)
        continue
      }
      for (const [selector, value] of Object.entries(step.fill ?? {})) {
        const set = await this.evaluate(
          `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return false; el.value = ${JSON.stringify(value)}; return true })()`,
        )
        if (!set) problems.push(`no element matches ${selector}`)
      }
      const found = await this.evaluate(`!!document.querySelector(${JSON.stringify(step.click)})`)
      if (!found) {
        problems.push(`no element matches ${step.click}`)
        break
      }
      status = await this.navigate(null, problems, () =>
        this.evaluate(`document.querySelector(${JSON.stringify(step.click)}).click()`),
      )
    }

    if (page.exercise) {
      try { await page.exercise(this) } catch (error) { problems.push(error.message) }
    }
    const expected = page.status ?? 200
    if (status !== expected) problems.push(`status ${status}, expected ${expected}`)

    const facts = await this.evaluate(`(async () => {
      await document.fonts.ready
      await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))
      const faces = [...document.fonts]
      // Card photos are lazy; decode() fetches them wherever they sit.
      const cardPhotos = [...document.querySelectorAll('.listing-item.has-photos img')]
      const photosBroken = (await Promise.all(cardPhotos.map(i => i.decode().then(() => false, () => true)))).filter(Boolean).length
      return {
        path: location.pathname,
        cardPhotos: cardPhotos.length,
        photosBroken,
        scrollWidth: document.documentElement.scrollWidth,
        logotype: !!document.querySelector(${JSON.stringify(LOGOTYPE_SELECTOR)}),
        loaded: [...new Set(faces.filter(f => f.status === 'loaded').map(f => f.family.replaceAll('"', '')))],
        failed: faces.filter(f => f.status === 'error').map(f => f.family + ' ' + f.style + ' ' + f.weight),
        expected: ${JSON.stringify(page.expect ?? null)} === null || !!document.querySelector(${JSON.stringify(page.expect ?? '')}),
      }
    })()`)

    // A page that quietly redirects (a signed-in page bouncing to /login)
    // must not pass as itself.
    const finalPath = page.finalPath ?? new URL(page.path, BASE).pathname
    if (facts.path !== finalPath) problems.push(`ended at ${facts.path}, expected ${finalPath}`)
    if (!facts.expected) problems.push(`nothing matches ${page.expect}`)
    if (facts.photosBroken) problems.push(`${facts.photosBroken} of ${facts.cardPhotos} card photos did not load`)
    // Measured against the emulated width, not innerWidth: in phone
    // emulation Chrome widens the layout viewport to fit oversized content,
    // which would hide exactly the overflow this looks for.
    if (facts.scrollWidth > viewport.width) {
      problems.push(`scrolls horizontally: ${facts.scrollWidth}px of content in ${viewport.width}px`)
    }
    const families = facts.logotype ? [...FONT_FAMILIES, LOGOTYPE_FAMILY] : FONT_FAMILIES
    for (const family of families) {
      if (!facts.loaded.includes(family)) problems.push(`font not loaded: ${family}`)
    }
    for (const face of facts.failed) problems.push(`font failed: ${face}`)

    const metrics = await this.session.send('Page.getLayoutMetrics')
    const { width, height } = metrics.cssContentSize
    const shot = await this.session.send('Page.captureScreenshot', {
      format: 'png',
      captureBeyondViewport: true,
      clip: { x: 0, y: 0, width: viewport.width, height: Math.ceil(height || width), scale: 1 },
    })
    await writeFile(path.join(OUT, `${page.name}-${viewport.label}.png`), Buffer.from(shot.data, 'base64'))
    return problems
  }

  /**
   * Navigate (to `url`, or by running `trigger`) and wait for the load
   * event. Returns the main document's HTTP status; console errors,
   * exceptions, CSP violations, and failed subresources go into `problems`.
   */
  // Whether `selector` matches within four seconds.
  async appears(selector) {
    return this.evaluate(`(async () => {
      for (let i = 0; i < 40; i++) {
        if (document.querySelector(${JSON.stringify(selector)})) return true
        await new Promise(r => setTimeout(r, 100))
      }
      return false
    })()`)
  }

  async navigate(url, problems = [], trigger) {
    const { session } = this
    let status = null
    let documentRequest = null
    const off = [
      session.on('Network.responseReceived', ({ requestId, type, response }) => {
        if (type === 'Document') {
          documentRequest = requestId
          status = response.status
        } else if (response.status >= 400 && !response.url.endsWith('/favicon.ico')) {
          problems.push(`${response.status} for ${response.url}`)
        }
      }),
      session.on('Network.loadingFailed', ({ requestId, errorText, canceled }) => {
        if (requestId !== documentRequest && !canceled) problems.push(`request failed: ${errorText}`)
      }),
      session.on('Runtime.exceptionThrown', ({ exceptionDetails }) => {
        problems.push(`exception: ${exceptionDetails.exception?.description ?? exceptionDetails.text}`)
      }),
      session.on('Runtime.consoleAPICalled', ({ type, args }) => {
        if (type === 'error') problems.push(`console.error: ${args.map((a) => a.value ?? a.description).join(' ')}`)
      }),
      session.on('Log.entryAdded', ({ entry }) => {
        // Network failures are reported above, with the favicon excluded
        // and the page's own status judged separately.
        if (entry.level === 'error' && entry.source !== 'network') {
          problems.push(`${entry.source}: ${entry.text}`)
        }
      }),
    ]
    try {
      const loaded = session.once('Page.loadEventFired', NAVIGATION_TIMEOUT_MS)
      if (url) {
        const result = await session.send('Page.navigate', { url })
        if (result.errorText) throw new Error(`could not load ${url}: ${result.errorText}`)
      } else {
        await trigger()
      }
      await loaded
      // Let late console messages and font requests arrive.
      await sleep(250)
    } finally {
      off.forEach((unsubscribe) => unsubscribe())
    }
    return status
  }

  async evaluate(expression) {
    const { result, exceptionDetails } = await this.session.send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true,
    })
    if (exceptionDetails) throw new Error(`evaluation failed: ${exceptionDetails.text}`)
    return result.value
  }

  async close() {
    this.session.close()
  }
}

class Session {
  static async connect(url) {
    const ws = new WebSocket(url)
    await new Promise((resolve, reject) => {
      ws.addEventListener('open', resolve, { once: true })
      ws.addEventListener('error', () => reject(new Error('could not connect to Chrome')), { once: true })
    })
    return new Session(ws)
  }

  constructor(ws) {
    this.ws = ws
    this.nextId = 0
    this.pending = new Map()
    this.listeners = new Map()
    ws.addEventListener('message', (event) => {
      const msg = JSON.parse(event.data)
      if (msg.id !== undefined) {
        const waiter = this.pending.get(msg.id)
        this.pending.delete(msg.id)
        if (!waiter) return
        if (msg.error) waiter.reject(new Error(`${waiter.method}: ${msg.error.message}`))
        else waiter.resolve(msg.result)
      } else {
        for (const listener of this.listeners.get(msg.method) ?? []) listener(msg.params)
      }
    })
  }

  send(method, params = {}) {
    const id = ++this.nextId
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, method })
      this.ws.send(JSON.stringify({ id, method, params }))
    })
  }

  on(method, listener) {
    if (!this.listeners.has(method)) this.listeners.set(method, new Set())
    this.listeners.get(method).add(listener)
    return () => this.listeners.get(method).delete(listener)
  }

  once(method, timeoutMs) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        off()
        reject(new Error(`timed out waiting for ${method}`))
      }, timeoutMs)
      const off = this.on(method, (params) => {
        clearTimeout(timer)
        off()
        resolve(params)
      })
    })
  }

  close() {
    this.ws.close()
  }
}

// ---- processes and plumbing ----

function findChrome() {
  const candidates = [
    process.env.CHROME,
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
  ].filter(Boolean)
  const found = candidates.find((candidate) => existsSync(candidate))
  if (!found) throw new Error('Chrome not found; set CHROME to its executable')
  return found
}

/** Run to completion; return stdout, or throw with stderr. */
function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: ROOT, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(`${path.basename(command)} ${args.join(' ')} failed:\n${result.stderr || result.stdout}`)
  }
  return result.stdout
}

/** Start a long-running child that is stopped on exit. */
function start(command, args, env, label) {
  const child = spawn(command, args, { cwd: ROOT, env, stdio: ['ignore', 'ignore', 'pipe'] })
  let stderr = ''
  child.stderr.on('data', (chunk) => {
    stderr = (stderr + chunk).slice(-4000)
  })
  child.label = label
  child.stderrTail = () => stderr
  children.push(child)
  return child
}

async function waitFor(url, child, message) {
  for (let i = 0; i < 150; i++) {
    if (child.exitCode !== null) throw new Error(`${message}:\n${child.stderrTail()}`)
    try {
      if ((await fetch(url)).ok) return
    } catch {
      // not listening yet
    }
    await sleep(200)
  }
  throw new Error(`${message} within 30s`)
}

async function expectOk(url, message) {
  try {
    if ((await fetch(url)).ok) return
  } catch {
    // fall through
  }
  throw new Error(message)
}

async function cleanup() {
  for (const child of children) {
    if (child.exitCode === null) child.kill('SIGTERM')
  }
  await sleep(300)
  if (tempProfile) await rm(tempProfile, { recursive: true, force: true }).catch(() => {})
}

function requireEnv(name) {
  const value = process.env[name]
  if (!value) throw new Error(`${name} is not set; run this through \`just visual-check\` after \`just dev-env\``)
  return value
}

function splitOnce(s, sep) {
  const i = s.indexOf(sep)
  if (i < 0) throw new Error(`unexpected dev-session output: ${s}`)
  return [s.slice(0, i), s.slice(i + 1)]
}
