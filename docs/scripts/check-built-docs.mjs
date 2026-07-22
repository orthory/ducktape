import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const contentRoot = join(root, 'src/content/docs')
const dist = join(root, 'dist')
const sources = walk(contentRoot).filter((path) => path.endsWith('.mdx'))
const failures = []

for (const source of sources) {
  const rel = relative(contentRoot, source).split(sep).join('/')
  const stem = rel.slice(0, -'.mdx'.length)
  const route = stem.endsWith('/index') ? stem.slice(0, -'/index'.length) : stem
  const output = join(dist, route)

  for (const file of ['index.html', 'index.md', 'index.mdx']) {
    if (!existsSync(join(output, file))) failures.push(`missing /${route}/${file}`)
  }
  const sourceText = readFileSync(source, 'utf8')
  const html = readFileSync(join(output, 'index.html'), 'utf8')
  if (sourceText.includes('client:visible') && !html.includes('client="visible"')) {
    failures.push(`/${route} lost its React island hydration`)
  }
}

for (const file of [
  'index.html',
  '404.html',
  'llms.txt',
  'llms-full.txt',
  'robots.txt',
  'sitemap-index.xml',
  'pagefind/pagefind.js',
  'opengraph.png',
  'favicon.svg',
]) {
  if (!existsSync(join(dist, file))) failures.push(`missing /${file}`)
}

const landing = readFileSync(join(dist, 'index.html'), 'utf8')
if (!landing.includes('component-export="Landing"') || !landing.includes('client="load"')) {
  failures.push('landing page lost its hydrated React surface')
}
if (!landing.includes('<option value="" selected disabled>Choose track</option>')) {
  failures.push('landing track picker must start with a navigable placeholder')
}

const notFound = readFileSync(join(dist, '404.html'), 'utf8')
if (!notFound.includes('id="main-content"')) failures.push('404 skip link must have a target')
if (notFound.includes('data-menu-btn')) failures.push('404 must not show a dead sidebar button')
if (!notFound.includes('<title>Page not found | Ducktape</title>')) failures.push('404 title must name Ducktape once')
if (!notFound.includes('<meta name="robots" content="noindex">')) failures.push('404 must be noindex')

const en = readFileSync(join(dist, 'en/human/index.html'), 'utf8')
const ko = readFileSync(join(dist, 'ko/human/index.html'), 'utf8')
if (!en.includes('<html lang="en"')) failures.push('English pages must declare lang="en"')
if (!ko.includes('<html lang="ko"')) failures.push('Korean pages must declare lang="ko"')
if (!en.includes('/opengraph.png')) failures.push('pages must advertise the static social card')
if (!en.includes('data-md-url="/en/human/index.md"') || !en.includes('href="/en/human/index.md"')) {
  failures.push('Markdown page actions must use a host-relative URL')
}
if (!/<div data-nb-page-actions[^>]*data-pagefind-ignore/.test(en)) {
  failures.push('Page actions must stay out of Pagefind snippets')
}
for (const label of ['Human · English', 'Human · 한국어', 'Agent · English', 'Agent · 한국어']) {
  if (!en.includes(label)) failures.push(`header is missing ${label}`)
}
assertSidebarOrder(en, [
  '/en/human/',
  '/en/human/start/quick-start/',
  '/en/human/architecture/platform-invariants/',
  '/en/human/architecture/module-model/',
  '/en/human/architecture/consensus-and-node/',
  '/en/human/architecture/async-engine/',
  '/en/human/architecture/state-sync/',
  '/en/human/network/network-and-membership/',
  '/en/human/network/node-upgrades/',
  '/en/human/network/coordination/',
  '/en/human/modules/product-modules/',
  '/en/human/roadmap/what-is-left/',
  '/en/human/reference/repository-map/',
  '/en/human/reference/implementation-status/',
  '/en/human/reference/design-records/',
  '/en/human/reference/gotchas/',
], 'English human')

const enAgent = readFileSync(join(dist, 'en/agent/index.html'), 'utf8')
assertSidebarOrder(enAgent, [
  '/en/agent/',
  '/en/agent/start/operating-loop/',
  '/en/agent/architecture/determinism-contract/',
  '/en/agent/architecture/state-sync-contract/',
  '/en/agent/network/validator-operations/',
  '/en/agent/network/node-upgrades/',
  '/en/agent/roadmap/open-work/',
  '/en/agent/reference/repository-map/',
  '/en/agent/reference/verification-matrix/',
  '/en/agent/reference/design-records/',
  '/en/agent/reference/gotchas/',
], 'English agent')

const pagination = en.slice(en.lastIndexOf('<nav aria-label="Pagination"'))
if (!pagination.includes('href="/en/human/start/quick-start/"')) {
  failures.push('human overview must paginate to Quick Start')
}

const css = walk(join(dist, '_astro'))
  .filter((path) => path.endsWith('.css'))
  .map((path) => readFileSync(path, 'utf8'))
  .join('\n')
if (!css.includes('Noto Sans KR Variable')) failures.push('built CSS must bundle Korean glyphs')

const markdownTwin = readFileSync(join(dist, 'en/human/index.md'), 'utf8')
const fullCorpus = readFileSync(join(dist, 'llms-full.txt'), 'utf8')
if (/^import /m.test(markdownTwin)) failures.push('Markdown twins must omit MDX imports')
if (/^import /m.test(fullCorpus)) failures.push('llms-full.txt must omit MDX imports')

const htmlPages = walk(dist).filter((path) => path.endsWith('.html'))
if (sources.length !== 54) failures.push(`expected 54 sources, found ${sources.length}`)
if (htmlPages.length !== 56) failures.push(`expected 56 HTML pages, found ${htmlPages.length}`)

if (failures.length) {
  console.error('built docs check failed:')
  for (const failure of failures) console.error(`- ${failure}`)
  process.exit(1)
}

console.log('built docs ok (54 routes + landing + 404, Markdown twins, social card, search, sitemap, hydrated diagrams)')

function walk(dir) {
  const files = []
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) files.push(...walk(path))
    else files.push(path)
  }
  return files
}

function assertSidebarOrder(html, routes, track) {
  const start = html.indexOf('<aside id="desktop-sidebar"')
  const end = html.indexOf('</aside>', start)
  const sidebar = html.slice(start, end)
  let cursor = -1
  for (const route of routes) {
    const next = sidebar.indexOf(`href="${route}"`, cursor + 1)
    if (next === -1) {
      failures.push(`${track} sidebar is missing or misorders ${route}`)
      return
    }
    cursor = next
  }
}
