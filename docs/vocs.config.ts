import { defineConfig } from 'vocs/config'

const humanEn = [
  { text: 'Overview', link: '/en/human' },
  { text: 'Quick Start', link: '/en/human/start/quick-start' },
  {
    text: 'Architecture',
    items: [
      { text: 'Platform Invariants', link: '/en/human/architecture/platform-invariants' },
      { text: 'Module Model', link: '/en/human/architecture/module-model' },
      { text: 'Consensus and Node', link: '/en/human/architecture/consensus-and-node' },
      { text: 'Async Engine', link: '/en/human/architecture/async-engine' },
      { text: 'State Sync', link: '/en/human/architecture/state-sync' },
    ],
  },
  { text: 'Network and Membership', link: '/en/human/network/network-and-membership' },
  { text: 'Product Modules', link: '/en/human/modules/product-modules' },
  { text: 'What Is Left', link: '/en/human/roadmap/what-is-left' },
  {
    text: 'Reference',
    items: [
      { text: 'Repository Map', link: '/en/human/reference/repository-map' },
      { text: 'Implementation Status', link: '/en/human/reference/implementation-status' },
      { text: 'Gotchas', link: '/en/human/reference/gotchas' },
    ],
  },
]

const humanKo = [
  { text: '개요', link: '/ko/human' },
  { text: 'Quick Start', link: '/ko/human/start/quick-start' },
  {
    text: 'Architecture',
    items: [
      { text: 'Platform Invariants', link: '/ko/human/architecture/platform-invariants' },
      { text: 'Module Model', link: '/ko/human/architecture/module-model' },
      { text: 'Consensus and Node', link: '/ko/human/architecture/consensus-and-node' },
      { text: 'Async Engine', link: '/ko/human/architecture/async-engine' },
      { text: 'State Sync', link: '/ko/human/architecture/state-sync' },
    ],
  },
  { text: 'Network and Membership', link: '/ko/human/network/network-and-membership' },
  { text: 'Product Modules', link: '/ko/human/modules/product-modules' },
  { text: 'What Is Left', link: '/ko/human/roadmap/what-is-left' },
  {
    text: 'Reference',
    items: [
      { text: 'Repository Map', link: '/ko/human/reference/repository-map' },
      { text: 'Implementation Status', link: '/ko/human/reference/implementation-status' },
      { text: 'Gotchas', link: '/ko/human/reference/gotchas' },
    ],
  },
]

const agentEn = [
  { text: 'Overview', link: '/en/agent' },
  { text: 'Operating Loop', link: '/en/agent/start/operating-loop' },
  {
    text: 'Contracts',
    items: [
      { text: 'Determinism Contract', link: '/en/agent/architecture/determinism-contract' },
      { text: 'State Sync Contract', link: '/en/agent/architecture/state-sync-contract' },
      { text: 'Validator Operations', link: '/en/agent/network/validator-operations' },
    ],
  },
  { text: 'Open Work', link: '/en/agent/roadmap/open-work' },
  {
    text: 'Reference',
    items: [
      { text: 'Repository Map', link: '/en/agent/reference/repository-map' },
      { text: 'Verification Matrix', link: '/en/agent/reference/verification-matrix' },
      { text: 'Gotchas', link: '/en/agent/reference/gotchas' },
    ],
  },
]

const agentKo = [
  { text: '개요', link: '/ko/agent' },
  { text: 'Operating Loop', link: '/ko/agent/start/operating-loop' },
  {
    text: 'Contracts',
    items: [
      { text: 'Determinism Contract', link: '/ko/agent/architecture/determinism-contract' },
      { text: 'State Sync Contract', link: '/ko/agent/architecture/state-sync-contract' },
      { text: 'Validator Operations', link: '/ko/agent/network/validator-operations' },
    ],
  },
  { text: 'Open Work', link: '/ko/agent/roadmap/open-work' },
  {
    text: 'Reference',
    items: [
      { text: 'Repository Map', link: '/ko/agent/reference/repository-map' },
      { text: 'Verification Matrix', link: '/ko/agent/reference/verification-matrix' },
      { text: 'Gotchas', link: '/ko/agent/reference/gotchas' },
    ],
  },
]

export default defineConfig({
  title: 'Ducktape',
  description:
    'A consensus-based workplace super-app built from isolated authenticated modules.',
  srcDir: '.',
  renderStrategy: 'full-static',
  aiCta: false,
  topNav: [
    { text: 'Human EN', link: '/en/human' },
    { text: 'Human KO', link: '/ko/human' },
    { text: 'Agent EN', link: '/en/agent' },
    { text: 'Agent KO', link: '/ko/agent' },
  ],
  sidebar: {
    '/en/human': { backLink: true, items: humanEn },
    '/ko/human': { backLink: true, items: humanKo },
    '/en/agent': { backLink: true, items: agentEn },
    '/ko/agent': { backLink: true, items: agentKo },
  },
})
