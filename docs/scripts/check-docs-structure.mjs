import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const humanPages = [
  'index.mdx',
  'start/quick-start.mdx',
  'architecture/platform-invariants.mdx',
  'architecture/module-model.mdx',
  'architecture/consensus-and-node.mdx',
  'architecture/async-engine.mdx',
  'architecture/state-sync.mdx',
  'network/network-and-membership.mdx',
  'modules/product-modules.mdx',
  'roadmap/what-is-left.mdx',
  'reference/repository-map.mdx',
  'reference/implementation-status.mdx',
  'reference/gotchas.mdx',
]
const agentPages = [
  'index.mdx',
  'start/operating-loop.mdx',
  'architecture/determinism-contract.mdx',
  'architecture/state-sync-contract.mdx',
  'network/validator-operations.mdx',
  'roadmap/open-work.mdx',
  'reference/repository-map.mdx',
  'reference/verification-matrix.mdx',
  'reference/gotchas.mdx',
]
const required = [
  'package.json',
  'bun.lock',
  'vocs.config.ts',
  'pages/index.mdx',
  ...humanPages.flatMap((page) => [`pages/en/human/${page}`, `pages/ko/human/${page}`]),
  ...agentPages.flatMap((page) => [`pages/en/agent/${page}`, `pages/ko/agent/${page}`]),
]

const failures = []

if (existsSync(join(root, 'docs'))) {
  failures.push('nested docs/docs directory must not exist')
}

if (existsSync(join(root, 'pnpm-lock.yaml'))) {
  failures.push('docs must use bun.lock, not pnpm-lock.yaml')
}

for (const file of required) {
  if (!existsSync(join(root, file))) failures.push(`missing ${file}`)
}

const packageJson = readFileSync(join(root, 'package.json'), 'utf8')
if (!/"packageManager":\s*"bun@/.test(packageJson)) {
  failures.push('package.json must declare bun as packageManager')
}
if (/\bpnpm\b/.test(packageJson)) {
  failures.push('package.json must not reference pnpm')
}

const config = readFileSync(join(root, 'vocs.config.ts'), 'utf8')
if (!/srcDir:\s*['"]\.['"]/.test(config)) {
  failures.push('vocs.config.ts must keep srcDir set to "."')
}
for (const route of ['/en/human', '/ko/human', '/en/agent', '/ko/agent']) {
  if (!config.includes(`'${route}'`)) {
    failures.push(`vocs.config.ts must include ${route}`)
  }
}

const readme = readFileSync(join(root, 'README.md'), 'utf8')
if (/\bpnpm\b/.test(readme)) {
  failures.push('README.md must use bun commands')
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
  if (/\bpnpm\b/.test(text)) {
    failures.push(`${rel} must use bun commands`)
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

console.log(`docs structure ok (${pages.length} pages across 4 reader/language tracks)`)
