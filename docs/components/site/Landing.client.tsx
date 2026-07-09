'use client'

import type { ReactNode } from 'react'
import './landing.css'

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
          <a key={l.href} className="dt-track__link" href={l.href}>
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
