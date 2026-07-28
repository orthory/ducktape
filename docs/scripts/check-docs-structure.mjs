import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const contentRoot = join(root, 'src/content/docs')
const humanPages = [
  'index.mdx',
  'start/quick-start.mdx',
  'architecture/platform-invariants.mdx',
  'architecture/module-model.mdx',
  'architecture/consensus-and-node.mdx',
  'architecture/async-engine.mdx',
  'architecture/state-sync.mdx',
  'network/network-and-membership.mdx',
  'network/coordination.mdx',
  'modules/product-modules.mdx',
  'roadmap/what-is-left.mdx',
  'reference/repository-map.mdx',
  'reference/implementation-status.mdx',
  'reference/design-records.mdx',
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
  'reference/design-records.mdx',
  'reference/gotchas.mdx',
]
const required = [
  'package.json',
  'bun.lock',
  'astro.config.ts',
  'tsconfig.json',
  'src/components.ts',
  'src/content.config.ts',
  'src/layouts/BaseLayout.astro',
  'src/layouts/DocsLayout.astro',
  'src/layouts/LandingLayout.astro',
  'src/pages/index.mdx',
  ...humanPages.map((page) => `src/content/docs/en/human/${page}`),
  ...agentPages.map((page) => `src/content/docs/en/agent/${page}`),
]

const failures = []

if (existsSync(join(root, 'docs'))) {
  failures.push('nested docs/docs directory must not exist')
}
if (existsSync(join(root, 'pages'))) {
  failures.push('Vocs-era docs/pages must not exist')
}
if (existsSync(join(root, 'vocs.config.ts'))) {
  failures.push('Vocs config must not exist')
}

// dogfood.md is the maintained agent-loop guide created at this location by
// its design of record; all other maintained non-page records stay grouped.
const allowedRootMarkdown = new Set(['README.md', 'dogfood.md'])
for (const entry of readdirSync(root)) {
  if (entry.endsWith('.md') && !allowedRootMarkdown.has(entry)) {
    failures.push(`root-level ${entry} must live under docs/records, docs/adr, or docs/deploy`)
  }
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
if (!packageJson.includes('"@cloudflare/nimbus-docs"')) {
  failures.push('package.json must depend on @cloudflare/nimbus-docs')
}
if (!packageJson.includes('"build": "astro build"')) {
  failures.push('package.json must build with Astro')
}
if (/\bvocs\b/i.test(packageJson)) {
  failures.push('package.json must not reference Vocs')
}
if (/\bpnpm\b/.test(packageJson)) {
  failures.push('package.json must not reference pnpm')
}

const config = readFileSync(join(root, 'astro.config.ts'), 'utf8')
for (const route of ['en/human', 'en/agent']) {
  if (!config.includes(`segment: "/${route}"`)) {
    failures.push(`astro.config.ts must include the ${route} navigation track`)
  }
}
for (const label of ['Human · English', 'Agent · English']) {
  if (!config.includes(`label: "${label}"`)) {
    failures.push(`astro.config.ts must include navigation label ${label}`)
  }
}
for (const integration of ['react()', 'nimbus(nimbusConfig']) {
  if (!config.includes(integration)) {
    failures.push(`astro.config.ts must include ${integration}`)
  }
}

const readme = readFileSync(join(root, 'README.md'), 'utf8')
if (/\bpnpm\b/.test(readme)) {
  failures.push('README.md must use bun commands')
}

const pages = []
function walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) walk(path)
    else if (path.endsWith('.mdx')) pages.push(path)
  }
}
walk(contentRoot)

for (const page of pages) {
  const text = readFileSync(page, 'utf8')
  const rel = relative(root, page)
  const frontmatter = text.match(/^---\n([\s\S]*?)\n---/)
  if (!frontmatter || !/^title:\s*\S/m.test(frontmatter[1])) {
    failures.push(`${rel} has no title frontmatter`)
  }
  if (rel.includes('/human/') && !/^audience:\s*human\s*$/m.test(frontmatter?.[1] ?? '')) {
    failures.push(`${rel} must declare the human audience`)
  }
  if (/\b(TODO|TBD|PLACEHOLDER)\b/i.test(text)) {
    failures.push(`${rel} contains placeholder language`)
  }
  if (/\bpnpm\b/.test(text)) {
    failures.push(`${rel} must use bun commands`)
  }
  if (/\b(showAskAi|layout:\s*docs)\b/.test(text)) {
    failures.push(`${rel} contains Vocs frontmatter`)
  }
  if (/<(?:ArchitectureStack|AsyncEngine|ConsensusFlow|InvitationFlow|ModuleLifecycle|ModuleMap|NetworkTopology|StateSyncFlow|WorkspaceMap)\s*\/>/.test(text)) {
    failures.push(`${rel} has an unhydrated React diagram`)
  }
}

const landing = readFileSync(join(root, 'src/pages/index.mdx'), 'utf8')
if (!/^title:\s*Ducktape\s*$/m.test(landing) || !/^layout:\s*\.\.\/layouts\/LandingLayout\.astro\s*$/m.test(landing)) {
  failures.push('src/pages/index.mdx must use the Ducktape Nimbus landing layout')
}

if (pages.length !== 25) {
  failures.push(`expected 25 routed content pages, found ${pages.length}`)
}

if (failures.length) {
  console.error('docs structure check failed:')
  for (const failure of failures) console.error(`- ${failure}`)
  process.exit(1)
}

console.log('docs structure ok (25 pages across 2 reader tracks + landing)')
