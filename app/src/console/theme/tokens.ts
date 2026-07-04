// Design tokens carried over from the Ducktape Console design source.
// Components reference these for shared surfaces; section-specific one-off
// colors may still be written inline to stay faithful to the source.

export const color = {
  // text / ink
  ink: "#2c2b27",
  inkSoft: "#3f3e39",
  inkSofter: "#4a4843",
  muted: "#878787",
  muted2: "#a1a1a1",
  muted3: "#676767",
  iconIdle: "#c5c5c5",

  // surfaces
  paper: "#ffffff",
  sidebar: "#f9f9f9",
  titlebar: "#f1f1f1",
  hover: "#ededed",
  sunken: "#f5f5f5",
  panel: "#efefef",

  // borders
  windowBorder: "#d1d1d1",
  border: "#e5e5e5",
  borderSoft: "#ececec",
  borderStrong: "#d6d6d6",
  chip: "#e3e3e3",

  // dark
  dark: "#26251f",
  onDark: "#efefef",

  // accent (overridable via --accent CSS var)
  accent: "#a05a3c",
  accentAlt1: "#3d63b8",
  accentAlt2: "#3f7d54",

  // status
  green: "#5cb45f",
  amber: "#c08a3e",
  blue: "#5f7a9e",
  red: "#a35248",
  purple: "#7a6f9e",

  // destructive actions (delete confirm)
  danger: "#c0483c",
  dangerSoft: "#faf1ef",
  dangerBorder: "#eccbc5",
} as const;

export const font = {
  sans: "'Geist Sans', 'IBM Plex Sans KR', system-ui, -apple-system, sans-serif",
  mono: "'Geist Mono', ui-monospace, monospace",
} as const;

export const radius = {
  window: 13,
  lg: 11,
  md: 9,
  sm: 7,
} as const;

export const shadow = {
  window: "0 26px 72px rgba(40,38,34,.22), 0 4px 14px rgba(40,38,34,.10)",
  pop: "0 18px 48px rgba(40,38,34,.20), 0 3px 10px rgba(40,38,34,.10)",
  card: "0 1px 2px rgba(40,38,34,.05)",
} as const;

/** Reads the live accent (set on :root via --accent). Falls back to the default. */
export const accentVar = "var(--accent, #a05a3c)";
