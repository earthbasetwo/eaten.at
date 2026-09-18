#!/usr/bin/env node
// Export one page of the app as a single self-contained HTML file: the
// server's own markup with the stylesheet inlined and every web font
// embedded, so the file opens and looks right on its own, away from the
// app — a page to hand to a designer, or to a design tool.
//
//   just export-page                 # the editor for the first seeded write-up
//   just export-page /write          # any path, signed in
//   just export-page /write/abc out.html
//
// It runs against the local atproto network that `just dev-env` starts,
// on its own port (EXPORT_PORT, default 3101), so a `just run-dev` on
// :3000 is left alone. Signing in goes through `dev-session`, as the
// visual check does: no password, no OAuth screens.
//
// Links in the exported page still point at the app's paths, and so go
// nowhere from a file; everything that decides how the page looks —
// markup, CSS, fonts, the editor's own scripts — comes with it.

import { spawn, spawnSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'

const HERE = path.dirname(new URL(import.meta.url).pathname)
const ROOT = path.resolve(HERE, '..')
const OUT = path.join(ROOT, 'target', 'export')
const PORT = Number(process.env.EXPORT_PORT ?? 3101)
const BASE = `http://127.0.0.1:${PORT}`

const children = []

main()
  .then(cleanup)
  .catch(async (err) => {
    console.error(`export-page: ${err.message}`)
    await cleanup()
    process.exit(1)
  })

async function main() {
  const did = requireEnv('EATEN_AT_DEV_ALICE_DID')
  const pds = requireEnv('EATEN_AT_DEV_PDS')
  if (!process.env.EATEN_AT_DEV_INSECURE) {
    throw new Error('the dev environment is not loaded; run this through `just export-page`')
  }
  await expectOk(`${pds}/xrpc/_health`, 'the local PDS is not answering; start it with `just dev-env`')

  run('cargo', ['build', '-q', '-p', 'eaten-at', '--bin', 'eaten-at', '--bin', 'dev-session'])
  const env = {
    ...process.env,
    EATEN_AT_LISTEN: `127.0.0.1:${PORT}`,
    EATEN_AT_PUBLIC_URL: BASE,
    RUST_LOG: process.env.RUST_LOG ?? 'warn',
  }
  const app = start(path.join(ROOT, 'target', 'debug', 'eaten-at'), [], env, 'app')
  await waitFor(`${BASE}/healthz`, app, 'the app did not start')
  const cookie = run(path.join(ROOT, 'target', 'debug', 'dev-session'), [did], env).trim()

  const [pagePath, outArg] = args()
  const wanted = pagePath ?? `/write/${await firstRkey(did, cookie)}`
  const html = await inlined(await get(wanted, cookie), cookie)
  const file = path.resolve(OUT, outArg ?? `${slug(wanted)}.html`)
  await mkdir(path.dirname(file), { recursive: true })
  await writeFile(file, html)
  console.log(`${wanted} → ${path.relative(ROOT, file)} (${Math.round(html.length / 1024)} KB)`)
}

/// The path to export and the file to write it to, both optional. An
/// empty path is no path: `just export-page "" name.html` names the file
/// and leaves the page to the default.
function args() {
  const rest = process.argv.slice(2).filter((arg) => arg !== '')
  const pagePath = rest[0]?.startsWith('/') ? rest.shift() : undefined
  if (rest[0]?.startsWith('/')) throw new Error(`a file name, not a path: ${rest[0]}`)
  return [pagePath, rest[0]]
}

/// The record key of the author's most recent write-up, for the default
/// export: the editor with a document in it.
async function firstRkey(did, cookie) {
  const front = await get(`/at/${did}/`, cookie)
  const links = [...front.matchAll(/href="(\/at\/[^"]+)"/g)].map((m) => m[1])
  const doc = links.find((href) => {
    const parts = href.split('/').filter(Boolean)
    // /at/{did}/{publication}/{rkey}, and not the feed beside them.
    return parts.length === 4 && !parts[3].includes('.')
  })
  if (!doc) throw new Error(`no write-up is listed on /at/${did}/; seed the dev network first`)
  return doc.split('/').filter(Boolean).pop()
}

/// The page with its stylesheet folded into a <style> element and every
/// font the stylesheet asks for embedded as a data URL.
async function inlined(html, cookie) {
  const link = html.match(/<link rel="stylesheet" href="([^"]+)">/)
  if (!link) throw new Error('the page carries no stylesheet')
  let css = await get(link[1], cookie)
  for (const url of new Set([...css.matchAll(/url\("(\/static\/[^"]+)"\)/g)].map((m) => m[1]))) {
    css = css.split(`url("${url}")`).join(`url("${await dataUrl(url, cookie)}")`)
  }
  return html.replace(link[0], `<style>\n${css}\n</style>`)
}

/// One asset as a data URL, so the exported file asks the network for
/// nothing.
async function dataUrl(assetPath, cookie) {
  const response = await fetch(`${BASE}${assetPath}`, { headers: { cookie } })
  if (!response.ok) throw new Error(`${assetPath} answered ${response.status}`)
  const type = response.headers.get('content-type') ?? 'application/octet-stream'
  const body = Buffer.from(await response.arrayBuffer())
  return `data:${type};base64,${body.toString('base64')}`
}

async function get(pagePath, cookie) {
  const response = await fetch(`${BASE}${pagePath}`, { headers: { cookie } })
  if (!response.ok) throw new Error(`${pagePath} answered ${response.status}`)
  return response.text()
}

/// A file name for a page path: "/write/abc" becomes "write-abc".
function slug(pagePath) {
  const name = pagePath.replace(/[?#].*$/, '').split('/').filter(Boolean).join('-')
  return name || 'index'
}

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: ROOT, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(`${path.basename(command)} ${args.join(' ')} failed:\n${result.stderr || result.stdout}`)
  }
  return result.stdout
}

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
}

function requireEnv(name) {
  const value = process.env[name]
  if (!value) throw new Error(`${name} is not set; run this through \`just export-page\` after \`just dev-env\``)
  return value
}
