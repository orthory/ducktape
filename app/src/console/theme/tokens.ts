// Design tokens carried over from the Ducktape Console design source.
// Components reference these for shared surfaces; section-specific one-off
// colors may still be written inline to stay faithful to the source.
//
// Every token resolves through a `--c-*` CSS variable so the whole app can
// switch light ⇄ dark by flipping `data-theme` on <html> (see global.css and
// the theme effect in DucktapeProvider). The hard-coded hex is the light-mode
// fallback for any context where the var isn't set yet. Section-specific inline
// hexes scattered in components do NOT switch — they're the known dark-mode
// polish ceiling.
const v = (name: string, light: string) => `var(--c-${name}, ${light})`;

export const color = {
  // text / ink
  ink: v("ink", "#2c2b27"),
  inkSoft: v("ink-soft", "#3f3e39"),
  inkSofter: v("ink-softer", "#4a4843"),
  muted: v("muted", "#878787"),
  muted2: v("muted2", "#a1a1a1"),
  muted3: v("muted3", "#676767"),
  iconIdle: v("icon-idle", "#c5c5c5"),

  // surfaces
  paper: v("paper", "#ffffff"),
  sidebar: v("sidebar", "#f9f9f9"),
  titlebar: v("titlebar", "#f1f1f1"),
  hover: v("hover", "#ededed"),
  sunken: v("sunken", "#f5f5f5"),
  panel: v("panel", "#efefef"),

  // borders
  windowBorder: v("window-border", "#d1d1d1"),
  border: v("border", "#e5e5e5"),
  borderSoft: v("border-soft", "#ececec"),
  borderStrong: v("border-strong", "#d6d6d6"),
  chip: v("chip", "#e3e3e3"),

  // "filled" high-contrast swatch (active buttons/badges). Inverts in dark so a
  // filled control stays high-contrast against the surface.
  dark: v("filled", "#26251f"),
  onDark: v("on-filled", "#efefef"),

  // accent (overridable via --accent CSS var) — same in both themes
  accent: "#a05a3c",
  accentAlt1: "#3d63b8",
  accentAlt2: "#3f7d54",

  // status
  green: v("green", "#5cb45f"),
  amber: v("amber", "#c08a3e"),
  blue: v("blue", "#5f7a9e"),
  red: v("red", "#a35248"),
  purple: v("purple", "#7a6f9e"),

  // destructive actions (delete confirm)
  danger: v("danger", "#c0483c"),
  dangerSoft: v("danger-soft", "#faf1ef"),
  dangerBorder: v("danger-border", "#eccbc5"),
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
