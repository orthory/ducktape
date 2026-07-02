import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const root = new URL('..', import.meta.url).pathname
const required = [
  'package.json',
  'vocs.config.ts',
  'pages/index.mdx',
  'pages/start/quick-start.mdx',
  'pages/architecture/platform-invariants.mdx',
  'pages/architecture/module-model.mdx',
  'pages/architecture/consensus-and-node.mdx',
  'pages/architecture/async-engine.mdx',
  'pages/architecture/state-sync.mdx',
  'pages/network/network-and-membership.mdx',
  'pages/modules/product-modules.mdx',
  'pages/roadmap/what-is-left.mdx',
  'pages/reference/repository-map.mdx',
  'pages/reference/implementation-status.mdx',
  'pages/reference/gotchas.mdx',
]

const failures = []

if (existsSync(join(root, 'docs'))) {
  failures.push('nested docs/docs directory must not exist')
}

for (const file of required) {
  if (!existsSync(join(root, file))) failures.push(`missing ${file}`)
}

const config = readFileSync(join(root, 'vocs.config.ts'), 'utf8')
if (!/srcDir:\s*['"]\.['"]/.test(config)) {
  failures.push('vocs.config.ts must keep srcDir set to "."')
}

const pagesDir = join(root, 'pages')
const pages = []
function walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) walk(path)
    else if (path.endsWith('.mdx')) pages.push(path)
  }
}
walk(pagesDir)

for (const page of pages) {
  const text = readFileSync(page, 'utf8')
  const rel = relative(root, page)
  if (/\b(TODO|TBD|PLACEHOLDER)\b/i.test(text)) {
    failures.push(`${rel} contains placeholder language`)
  }
  if (!/^#\s+/m.test(text)) {
    failures.push(`${rel} has no h1`)
  }
}

if (failures.length) {
  console.error('docs structure check failed:')
  for (const failure of failures) console.error(`- ${failure}`)
  process.exit(1)
}

console.log(`docs structure ok (${pages.length} pages)`)
