import { defineConfig } from 'vocs/config'

export default defineConfig({
  title: 'Ducktape',
  description:
    'A consensus-based workplace super-app built from isolated authenticated modules.',
  srcDir: '.',
  renderStrategy: 'full-static',
  aiCta: false,
  sidebar: [
    {
      text: 'Start',
      items: [
        { text: 'Overview', link: '/' },
        { text: 'Quick Start', link: '/start/quick-start' },
      ],
    },
    {
      text: 'Architecture',
      items: [
        { text: 'Platform Invariants', link: '/architecture/platform-invariants' },
        { text: 'Module Model', link: '/architecture/module-model' },
        { text: 'Consensus and Node', link: '/architecture/consensus-and-node' },
        { text: 'Async Engine', link: '/architecture/async-engine' },
        { text: 'State Sync', link: '/architecture/state-sync' },
      ],
    },
    {
      text: 'Network',
      items: [
        { text: 'Network and Membership', link: '/network/network-and-membership' },
      ],
    },
    {
      text: 'Modules',
      items: [
        { text: 'Product Modules', link: '/modules/product-modules' },
      ],
    },
    {
      text: 'Roadmap',
      items: [
        { text: 'What Is Left', link: '/roadmap/what-is-left' },
      ],
    },
    {
      text: 'Reference',
      items: [
        { text: 'Repository Map', link: '/reference/repository-map' },
        { text: 'Implementation Status', link: '/reference/implementation-status' },
        { text: 'Gotchas', link: '/reference/gotchas' },
      ],
    },
  ],
})
