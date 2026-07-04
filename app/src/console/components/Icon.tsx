import type { CSSProperties, ReactNode } from "react";

// Line-style icon set (stroke = currentColor) carried over from the design
// source, trimmed to the icons this console uses.
const PATHS: Record<string, ReactNode> = {
  chat: <path d="M5 7a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-6l-4 3.5V14H7a2 2 0 0 1-2-2z" />,
  tasks: (
    <>
      <path d="M12 3.4l6.6 2.3v5c0 4.2-2.8 7-6.6 8.5-3.8-1.5-6.6-4.3-6.6-8.5v-5z" />
      <path d="M9.2 11.7l2 2 3.6-3.8" />
    </>
  ),
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
  modules: (
    <>
      <rect x="4.5" y="4.5" width="6" height="6" rx="1.4" />
      <rect x="13.5" y="4.5" width="6" height="6" rx="1.4" />
      <rect x="4.5" y="13.5" width="6" height="6" rx="1.4" />
      <rect x="13.5" y="13.5" width="6" height="6" rx="1.4" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 4v2M12 18v2M4 12h2M18 12h2M6.3 6.3l1.4 1.4M16.3 16.3l1.4 1.4M17.7 6.3l-1.4 1.4M7.7 16.3l-1.4 1.4" />
    </>
  ),
  close: <path d="M6 6l12 12M18 6L6 18" />,
  plus: <path d="M12 5v14M5 12h14" />,
  check: <path d="M5 12.5l4 4 10-10" />,
  chevronRight: <path d="M9 6l6 6-6 6" />,
  hash: <path d="M9 4L7 20M17 4l-2 16M5 9h15M4 15h15" />,
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
  agent: (
    <>
      <rect x="5" y="8" width="14" height="11" rx="3" />
      <path d="M12 4.7V8" />
      <circle cx="12" cy="4" r="1.1" />
      <circle cx="9.6" cy="13.3" r="1" />
      <circle cx="14.4" cy="13.3" r="1" />
    </>
  ),
  telemetry: <path d="M3 12.5h3.5l2-5.5 3 12 2.5-8.5 1.5 2h4.5" />,
  folder: <path d="M4 6.5a1.5 1.5 0 0 1 1.5-1.5h3.2l1.8 2h7A1.5 1.5 0 0 1 20 8.5v8a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 16.5z" />,
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
