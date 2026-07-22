'use client'

import type { ReactNode } from 'react'
import './landing.css'
import { ArchitectureStack } from '../diagram/ArchitectureStack.client'

/* Ducktape docs — landing/hero component kit. Presentational client components
 * so their CSS is bundled + injected by Vite; composed from pages/index.mdx. */

export function Hero({
  eyebrow,
  title,
  subtitle,
  actions,
  children,
}: {
  eyebrow?: string
  title: ReactNode
  subtitle?: ReactNode
  actions?: ReactNode
  children?: ReactNode
}) {
  return (
    <header className="dt-hero">
      <div className="dt-hero__glow" aria-hidden="true" />
      <div className="dt-hero__copy">
        {eyebrow ? <p className="dt-hero__eyebrow">{eyebrow}</p> : null}
        <h1 className="dt-hero__title">{title}</h1>
        {subtitle ? <p className="dt-hero__subtitle">{subtitle}</p> : null}
        {actions ? <div className="dt-hero__actions">{actions}</div> : null}
      </div>
      {children ? <div className="dt-hero__visual">{children}</div> : null}
    </header>
  )
}

export function Action({
  href,
  children,
  variant = 'primary',
}: {
  href: string
  children: ReactNode
  variant?: 'primary' | 'secondary'
}) {
  return (
    <a className={`dt-action dt-action--${variant}`} href={href}>
      {children}
      {variant === 'primary' ? <span className="dt-action__arrow" aria-hidden="true">→</span> : null}
    </a>
  )
}

export function SectionHead({
  eyebrow,
  title,
  children,
}: {
  eyebrow?: string
  title: ReactNode
  children?: ReactNode
}) {
  return (
    <div className="dt-sectionhead">
      {eyebrow ? <p className="dt-sectionhead__eyebrow">{eyebrow}</p> : null}
      <h2 className="dt-sectionhead__title">{title}</h2>
      {children ? <div className="dt-sectionhead__sub">{children}</div> : null}
    </div>
  )
}

export function FeatureGrid({ columns = 3, children }: { columns?: 2 | 3; children: ReactNode }) {
  return <div className={`dt-grid dt-grid--${columns}`}>{children}</div>
}

export function Feature({
  icon,
  title,
  children,
}: {
  icon?: keyof typeof icons
  title: ReactNode
  children: ReactNode
}) {
  return (
    <div className="dt-feature">
      {icon ? <div className="dt-feature__icon">{icons[icon]}</div> : null}
      <h3 className="dt-feature__title">{title}</h3>
      <div className="dt-feature__body">{children}</div>
    </div>
  )
}

export function TrackCards({ children }: { children: ReactNode }) {
  return <div className="dt-tracks">{children}</div>
}

export function TrackCard({
  kind,
  title,
  children,
  links,
}: {
  kind: 'human' | 'agent'
  title: ReactNode
  children: ReactNode
  links: { label: string; href: string }[]
}) {
  return (
    <div className={`dt-track dt-track--${kind}`}>
      <div className="dt-track__badge">{kind === 'human' ? 'For Humans' : 'For Agents'}</div>
      <h3 className="dt-track__title">{title}</h3>
      <div className="dt-track__body">{children}</div>
      <div className="dt-track__links">
        {links.map((l) => (
          <a
            key={l.href}
            className="dt-track__link"
            href={l.href}
            aria-label={`${kind === 'human' ? 'Human' : 'Agent'} · ${l.label}`}
          >
            {l.label}
            <span aria-hidden="true">→</span>
          </a>
        ))}
      </div>
    </div>
  )
}

export function StatRow({ children }: { children: ReactNode }) {
  return <div className="dt-stats">{children}</div>
}

export function Stat({ value, label }: { value: ReactNode; label: ReactNode }) {
  return (
    <div className="dt-stat">
      <span className="dt-stat__value">{value}</span>
      <span className="dt-stat__label">{label}</span>
    </div>
  )
}

export function Landing() {
  return (
    <>
      <Hero
        eyebrow="Consensus-based workplace OS"
        title={<>A workplace super-app, <em>replicated by consensus.</em></>}
        subtitle="Ducktape is one BFT-replicated state machine that hosts isolated, authenticated product modules — pages, forge, chat, agent workflows. Each module owns its state and exposes a single 32-byte root; the host folds them into one app-hash that consensus commits."
        actions={(
          <>
            <Action href="/en/human">Explore the platform</Action>
            <Action href="/en/human/start/quick-start" variant="secondary">Quick start</Action>
          </>
        )}
      >
        <ArchitectureStack />
      </Hero>

      <StatRow>
        <Stat value="12" label="Product modules" />
        <Stat value="10" label="Kernel crates" />
        <Stat value="17" label="System modules" />
        <Stat value="1" label="Global app-hash" />
      </StatRow>

      <SectionHead eyebrow="The module rule" title="Isolation you can review, composition you can verify">
        Modules never link each other's implementation crates. That single rule is what
        keeps a super-app tractable: every feature is an isolated state machine, and the
        only things that cross a boundary are typed messages and a 32-byte root.
      </SectionHead>

      <FeatureGrid columns={3}>
        <Feature icon="module" title="Isolated modules">
          A module depends only on <code>sdk</code> and the types-only interface crates
          of modules it addresses. No implementation crate ever imports another's.
        </Feature>
        <Feature icon="agent" title="Host-routed messaging">
          Cross-module reads go through host-routed queries. Cross-module writes are
          emitted as messages the host drains as follow-up ops inside the same block.
        </Feature>
        <Feature icon="hash" title="One app-hash">
          The host folds every module's sorted root into a single global app-hash. If
          two nodes agree on the app-hash, they agree on every module's state.
        </Feature>
      </FeatureGrid>

      <SectionHead eyebrow="Platform" title="What makes Ducktape a platform, not an app">
        The kernel is small and opinionated: a module contract, an ordered node path, a
        BFT orderer, and an async seam for effects. Everything a team actually uses is a
        module on top.
      </SectionHead>

      <FeatureGrid columns={3}>
        <Feature icon="consensus" title="BFT consensus">
          A commonware Simplex orderer agrees on height, time, and the ordered ops that
          every validator applies deterministically.
        </Feature>
        <Feature icon="lock" title="Authenticated state">
          Each module commits an authenticated root — QMDB key-value, a git head, or
          canonical snapshot bytes — that the app-hash makes tamper-evident.
        </Feature>
        <Feature icon="sync" title="Verifiable state sync">
          Joiners rebuild committed state over the sync wire and land on the exact
          source app-hash before they ever apply a new block.
        </Feature>
        <Feature icon="agent" title="Agent-native">
          Runs, dispatch, and a deterministic saga seam let coding agents act as
          first-class participants in the same consensus loop as people.
        </Feature>
        <Feature icon="upgrade" title="Live module code swap">
          A consensus-governed code registry swaps a module&apos;s wasm at an agreed
          height, so module logic evolves without stopping the network.
        </Feature>
        <Feature icon="network" title="Self-hosted networking">
          NAT traversal, a userspace WireGuard overlay, and a rendezvous coordinator
          let a validator set form across home networks with zero exposed ports.
        </Feature>
      </FeatureGrid>

      <SectionHead eyebrow="Two reader tracks" title="Docs for the people building it — and the agents running it">
        Every topic is written twice, on purpose. Pick the track that matches how you
        read; switch languages from the top navigation.
      </SectionHead>

      <TrackCards>
        <TrackCard
          kind="human"
          title="Product & architecture"
          links={[
            { label: 'English', href: '/en/human' },
            { label: '한국어', href: '/ko/human' },
          ]}
        >
          Platform shape, the consensus core, the module model, product modules,
          networking, and the implementation frontier — explained without assuming you
          are about to edit code.
        </TrackCard>
        <TrackCard
          kind="agent"
          title="Operating notes for agents"
          links={[
            { label: 'English', href: '/en/agent' },
            { label: '한국어', href: '/ko/agent' },
          ]}
        >
          Tighter implementation maps: determinism and state-sync contracts, validator
          operations, verification commands, module boundaries, and the open work a
          coding agent can pick up.
        </TrackCard>
      </TrackCards>
    </>
  )
}

/* --------------------------------- icons ---------------------------------- */

const s = { width: 20, height: 20, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 1.7, strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const }

const icons = {
  module: (
    <svg {...s}><path d="M12 2 3 7v10l9 5 9-5V7z" /><path d="M3 7l9 5 9-5" /><path d="M12 12v10" /></svg>
  ),
  consensus: (
    <svg {...s}><circle cx="6" cy="6" r="2.4" /><circle cx="18" cy="6" r="2.4" /><circle cx="12" cy="18" r="2.4" /><path d="M8 7.5 16 7.5M7.2 8 11 15.6M16.8 8 13 15.6" /></svg>
  ),
  hash: (
    <svg {...s}><path d="M9 3 7 21M17 3l-2 18M4 8.5h16M3 15.5h16" /></svg>
  ),
  agent: (
    <svg {...s}><rect x="4" y="8" width="16" height="11" rx="3" /><path d="M12 4v4M8.5 13h.01M15.5 13h.01M9.5 16.5h5" /></svg>
  ),
  server: (
    <svg {...s}><rect x="3" y="4" width="18" height="7" rx="2" /><rect x="3" y="13" width="18" height="7" rx="2" /><path d="M7 7.5h.01M7 16.5h.01" /></svg>
  ),
  upgrade: (
    <svg {...s}><path d="M21 12a9 9 0 1 1-3-6.7" /><path d="M21 4v5h-5" /></svg>
  ),
  lock: (
    <svg {...s}><rect x="4.5" y="10" width="15" height="10" rx="2.4" /><path d="M8 10V7a4 4 0 0 1 8 0v3" /></svg>
  ),
  sync: (
    <svg {...s}><path d="M4 5.5A8 8 0 0 1 19.5 8" /><path d="M4 4v4h4" /><path d="M20 18.5A8 8 0 0 1 4.5 16" /><path d="M20 20v-4h-4" /></svg>
  ),
  network: (
    <svg {...s}><circle cx="12" cy="5" r="2.2" /><circle cx="5" cy="19" r="2.2" /><circle cx="19" cy="19" r="2.2" /><path d="M12 7.2 6 17M12 7.2 18 17M7 19h10" /></svg>
  ),
} as const
