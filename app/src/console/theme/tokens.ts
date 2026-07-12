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
  // the app "page" the cards sit on — one notch recessed from paper. Its own
  // token (not `sunken`) so inputs, which use `sunken`, still read as wells
  // recessed below the page in both themes.
  canvas: v("canvas", "#fcfcfc"),
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
  // Hover shade for a filled control: nudge the fill toward its own text. In
  // light that lightens the dark fill; in dark (fill is now light) it darkens
  // it — the right direction in both themes without a second inline hex.
  filledHover: "color-mix(in srgb, var(--c-filled, #26251f) 85%, var(--c-on-filled, #efefef))",

  // Video scrim — deliberately theme-INVARIANT, and deliberately NOT a `--c-*`
  // var. A name chip over a video frame, and the letterbox around it, must stay
  // dark with light text in BOTH themes: the video looks the same either way, so
  // there is nothing for the chip to invert *against*. `dark`/`onDark` (which
  // ARE `--c-filled`/`--c-on-filled`) invert with the theme — using them here is
  // what made participant names vanish and video letterbox in near-white.
  scrim: "#26251f",
  // the same scrim, translucent, so the video reads faintly through a chip.
  scrimSoft: "rgba(38, 37, 31, 0.62)",
  onScrim: "#efefef",

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

/** A tinted status chip derived from one hue: vivid-but-readable text over a
 *  faint wash of the same color, with a slightly stronger border. Every part is
 *  mixed against the LIVE `--c-ink` / `--c-paper`, so a single call yields a
 *  pale-on-white chip in light mode and a dark-tinted chip in dark mode — no
 *  per-theme hexes. Pass a token (e.g. `color.green`), not a raw hex, so the
 *  base hue itself also shifts with the theme. */
export const tint = (base: string) => ({
  text: `color-mix(in srgb, ${base} 72%, var(--c-ink, #2c2b27))`,
  bg: `color-mix(in srgb, ${base} 14%, var(--c-paper, #ffffff))`,
  border: `color-mix(in srgb, ${base} 34%, var(--c-paper, #ffffff))`,
});

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
