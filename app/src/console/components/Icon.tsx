import type { CSSProperties, ReactNode } from "react";

// Line-style icon set (stroke = currentColor) carried over from the design
// source, trimmed to the icons this console uses.
const PATHS: Record<string, ReactNode> = {
  chat: <path d="M5 7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-6l-4 3.5V14H7a2 2 0 0 1-2-2z" />,
  arrowUp: <path d="M12 19V5M5 12l7-7 7 7" />,
  members: (
    <>
      <circle cx="10" cy="8" r="3" />
      <path d="M4.5 18c0-3 2.4-4.6 5.5-4.6 1 0 1.8.2 2.6.5" />
      <path d="M16 6.3a2.8 2.8 0 0 1 .3 5.4" />
      <path d="M17.6 13.7c1.9.5 2.9 1.9 2.9 3.9" />
    </>
  ),
  node: (
    <>
      <path d="M12 3.4l7.4 4.27v8.66L12 20.6l-7.4-4.27V7.67z" />
      <circle cx="12" cy="12" r="2.3" />
    </>
  ),
  sandbox: (
    <>
      <path d="M12 3.5l7 4v9l-7 4-7-4v-9z" />
      <path d="M5.3 7.7L12 12l6.7-4.3M12 12v8.2" />
    </>
  ),
  modules: (
    <>
      <rect x="4.5" y="4.5" width="6" height="6" rx="1.4" />
      <rect x="13.5" y="4.5" width="6" height="6" rx="1.4" />
      <rect x="4.5" y="13.5" width="6" height="6" rx="1.4" />
      <rect x="13.5" y="13.5" width="6" height="6" rx="1.4" />
    </>
  ),
  // Real cog (toothed ring) — deliberately NOT a rayed sun, so Settings can't
  // be mistaken for the light/dark theme toggle that lives beside it.
  settings: (
    <>
      <circle cx="12" cy="12" r="3.1" />
      <path d="M19.4 13a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V20a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 18.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H2.9a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.5 8a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 8.83 3.5H9a1.65 1.65 0 0 0 1-1.51V1.9a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 20.5 8v.17a1.65 1.65 0 0 0 1.51 1H22a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </>
  ),
  // sun / moon: the light/dark theme toggle glyphs.
  sun: (
    <>
      <circle cx="12" cy="12" r="4.2" />
      <path d="M12 2.5v2.4M12 19.1v2.4M4.2 4.2l1.7 1.7M18.1 18.1l1.7 1.7M2.5 12h2.4M19.1 12h2.4M4.2 19.8l1.7-1.7M18.1 5.9l1.7-1.7" />
    </>
  ),
  moon: <path d="M20.5 13.2A8 8 0 1 1 10.8 3.5a6.3 6.3 0 0 0 9.7 9.7z" />,
  bell: (
    <>
      <path d="M6.5 15.5v-5a5.5 5.5 0 0 1 11 0v5l1.5 2H5z" />
      <path d="M10.5 18.5a1.5 1.5 0 0 0 3 0" />
    </>
  ),
  search: (
    <>
      <circle cx="11" cy="11" r="6" />
      <path d="M15.5 15.5L20 20" />
    </>
  ),
  browser: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M3.8 9h16.4M8.7 3.9c-1.1 2.1-1.7 4.9-1.7 8.1s.6 6 1.7 8.1M15.3 3.9c1.1 2.1 1.7 4.9 1.7 8.1s-.6 6-1.7 8.1" />
    </>
  ),
  close: <path d="M6 6l12 12M18 6L6 18" />,
  plus: <path d="M12 5v14M5 12h14" />,
  check: <path d="M5 12.5l4 4 10-10" />,
  // double check — the finalization mark's "confirmed" glyph (cf. `check`,
  // its single-check "sent" counterpart). The ticks sit 10 units apart with
  // shortened tails: any tighter and they merge into one thick check at the
  // mark's 11px render size.
  checks: (
    <>
      <path d="M1.5 13l3.5 3.5 8-8.5" />
      <path d="M11.5 13l3.5 3.5 8-8.5" />
    </>
  ),
  code: (
    <>
      <path d="M9 18l-6-6 6-6" />
      <path d="M15 6l6 6-6 6" />
    </>
  ),
  edit: (
    <>
      <path d="M4.5 19.5l.8-3.6L15.2 6l2.8 2.8-9.9 9.9z" />
      <path d="M13.7 7.5l2.8 2.8" />
    </>
  ),
  divider: <path d="M5 12h14" />,
  link: (
    <>
      <path d="M9.5 14.5l5-5" />
      <path d="M11 7.4l1.4-1.4a4 4 0 0 1 5.7 5.7l-1.4 1.4" />
      <path d="M13 16.6l-1.4 1.4a4 4 0 0 1-5.7-5.7l1.4-1.4" />
    </>
  ),
  quote: (
    <>
      <path d="M8 9h4v7H6v-5.2A5 5 0 0 1 10.8 6" />
      <path d="M17 9h4v7h-6v-5.2A5 5 0 0 1 19.8 6" />
    </>
  ),
  chevronLeft: <path d="M15 6l-6 6 6 6" />,
  chevronRight: <path d="M9 6l6 6-6 6" />,
  hash: <path d="M9 4L7 20M17 4l-2 16M5 9h15M4 15h15" />,
  metrics: <path d="M6 20v-7M12 20V6M18 20v-4M4 20h16" />,
  forge: (
    <>
      <path d="M6 4.5v9" />
      <circle cx="6" cy="17.5" r="2.2" />
      <circle cx="18" cy="6.5" r="2.2" />
      <path d="M18 8.7a9 9 0 0 1-9 9" />
    </>
  ),
  document: (
    <>
      <path d="M7 3.5h6l4 4v13H7z" />
      <path d="M13 3.5v4h4" />
      <path d="M9.5 12h5M9.5 15.5h5" />
    </>
  ),
  pages: (
    <>
      <path d="M8.5 3.5h6l3.5 3.5v10.5h-9.5z" />
      <path d="M14.5 3.5V7H18" />
      <path d="M5.5 7v11.5a2 2 0 0 0 2 2H15" />
    </>
  ),
  agent: (
    <>
      <rect x="5" y="8" width="14" height="11" rx="3" />
      <path d="M12 4.7V8" />
      <circle cx="12" cy="4" r="1.1" />
      <circle cx="9.6" cy="13.3" r="1" />
      <circle cx="14.4" cy="13.3" r="1" />
    </>
  ),
  governance: (
    <>
      <circle cx="12" cy="12" r="8" />
      <path d="M8.5 12.2l2.4 2.4 4.6-4.8" />
    </>
  ),
  refresh: (
    <>
      <path d="M5.5 9.2A7 7 0 0 1 18 6.6l1.5 1.6" />
      <path d="M19.5 4v4h-4" />
      <path d="M18.5 14.8A7 7 0 0 1 6 17.4l-1.5-1.6" />
      <path d="M4.5 20v-4h4" />
    </>
  ),
  files: (
    <>
      <path d="M5 5.5A1.5 1.5 0 0 1 6.5 4H10l2 2.2h5.5A1.5 1.5 0 0 1 19 7.7V10" />
      <path d="M3.7 12.2A1.2 1.2 0 0 1 4.9 11h14.2a1.2 1.2 0 0 1 1.18 1.46l-1.1 6A1.5 1.5 0 0 1 17.7 20H6.3a1.5 1.5 0 0 1-1.48-1.24z" />
    </>
  ),
  // file-type glyphs for the forge tree (see views/forge/file-icons.ts)
  braces: (
    <>
      <path d="M9 4.5c-1.8 0-1.8 2.6-1.8 3.75 0 1.5-1.7 1.75-1.7 1.75s1.7.25 1.7 1.75c0 1.15 0 3.75 1.8 3.75" transform="translate(0 2)" />
      <path d="M15 4.5c1.8 0 1.8 2.6 1.8 3.75 0 1.5 1.7 1.75 1.7 1.75s-1.7.25-1.7 1.75c0 1.15 0 3.75-1.8 3.75" transform="translate(0 2)" />
    </>
  ),
  image: (
    <>
      <rect x="4" y="5" width="16" height="14" rx="2" />
      <circle cx="9" cy="10" r="1.5" />
      <path d="M4.5 17.5l4.5-4 3 2.5 3.2-3.2L19.5 16" />
    </>
  ),
};

export type IconName = keyof typeof PATHS;

export function Icon({
  name,
  size = 18,
  color = "currentColor",
  strokeWidth = 1.6,
  style,
}: {
  name: IconName;
  size?: number;
  color?: string;
  strokeWidth?: number;
  style?: CSSProperties;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ color, flexShrink: 0, ...style }}
      aria-hidden="true"
    >
      {PATHS[name]}
    </svg>
  );
}
