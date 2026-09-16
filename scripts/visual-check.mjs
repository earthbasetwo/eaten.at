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
    for (const viewport of WIDTHS) {
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
      { press: '.connect-button', wait: '#connect-handle:focus' },
      { type: { '#connect-handle': 'ali' }, wait: '#connect-handle-list [role="option"]' },
    ],
    expect: '.connect-idle[hidden] + form.connect-form:not([hidden])',
  })
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
    name: 'landing-typeahead',
    path: '/',
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
  const rkey = discovered.documents[0].split('/').pop()
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
  // The editor starts by choosing a place (plans 06, 12). The request is
  // located by EATEN_AT_DEV_LOCATION, so the search is on; the Search
  // button is the no-JS path and is clicked directly here.
  const search = (q) => ({ fill: { '#place_query': q }, click: 'button[name="action"][value="search"]' })
  const byHand = (name) => ({ fill: { '#place_name': name }, click: 'button[name="action"][value="manual"]' })
  await check({ name: 'write', path: '/write', expect: 'input[data-suggest]' })
  await check({ name: 'write-search', path: '/write', steps: [search('Noodle')], expect: '.result-item' })
  await check({ name: 'write-search-empty', path: '/write', steps: [search('nothing here')], expect: '.empty' })
  await check({ name: 'write-search-unavailable', path: '/write', steps: [search('quota')], expect: '.form-error' })
  // Suggestions as you type, and a pick through the listbox.
  await check({
    name: 'write-suggest',
    path: '/write',
    steps: [{ type: { '#place_query': 'noo' }, wait: '[role="option"]' }],
    expect: '[role="listbox"] [role="option"]',
  })
  await check({
    name: 'write-pick',
    path: '/write',
    steps: [{ type: { '#place_query': 'noo' }, wait: '[role="option"]' }, { click: '[role="option"]' }],
    expect: 'input[name="place_mode"][value="picked"]',
  })
  await check({
    name: 'write-pick-plain',
    path: '/write',
    steps: [search('Noodle'), { click: 'button[name="action"][value="pick:0"]' }],
    expect: 'input[name="place_mode"][value="picked"]',
  })
  await check({
    name: 'write-manual',
    path: '/write',
    steps: [byHand('The Cart')],
    expect: 'input[name="place_mode"][value="manual"]',
  })
  await check({
    name: 'write-manual-blank',
    path: '/write',
    steps: [{ click: 'button[name="action"][value="manual"]' }],
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
  await check({ name: 'edit', path: `/write/${rkey}` })
  await check({
    name: 'edit-change-place',
    path: `/write/${rkey}`,
    steps: [{ click: 'button[name="action"][value="change_place"]' }],
    expect: '#place_query[value]',
  })
  await check({
    name: 'edit-preview',
    path: `/write/${rkey}`,
    submit: 'form.editor button[name="action"][value="preview"]',
    expect: '.preview',
  })
  await check({ name: 'photos', path: `/write/${rkey}/photos`, expect: '.photo-row' })
  const bare = discovered.documents[discovered.documents.length - 1].split('/').pop()
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
  await check({ name: 'write-none', path: '/write', expect: '#place_query' })

  await browser.close()
  const total = failures.reduce((n, f) => n + f.problems.length, 0)
  console.log(
    failures.length
      ? `\n${failures.length} page view(s) with ${total} problem(s). Screenshots: ${path.relative(ROOT, OUT)}/`
      : `\nAll pages passed. Screenshots: ${path.relative(ROOT, OUT)}/`,
  )
  return failures.length > 0
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
