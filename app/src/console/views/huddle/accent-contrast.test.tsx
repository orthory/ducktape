// The accent-filled controls of the huddle surface, and the one rule they broke.
//
// `accentVar` is `var(--accent, …)` and is theme-INVARIANT: its only writer is
// DucktapeProvider (one persisted user hex) and nothing re-sets `--accent` under
// `data-theme=dark`. A background that does NOT invert must not carry a
// foreground that DOES — and every `--c-*` token inverts by construction. These
// four sites painted `color.onDark` (= `--c-on-filled`) on the accent: #efefef in
// light, but #1b1a17 in dark — near-black text on the brown accent, 3.33:1, under
// the 4.5:1 WCAG AA floor. Same bug class as the video scrim this PR exists to fix.
//
// jsdom cannot EVALUATE a CSS var (see global.css) — but it does keep one verbatim
// in an inline style, so the revert is directly observable: a reverted site renders
// `color: var(--c-on-filled, #efefef)`, and the "concrete color" assertion below
// rejects exactly that. Contrast is then computed against the accent's own literal
// default, which is sound precisely BECAUSE `--accent` never varies by theme.

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import type { Channel } from "../../../domain/chat-client";
import { ConsoleContext } from "../../store/context";
import type { ConsoleActions } from "../../store/actions";
import { createInitialState, type ConsoleState } from "../../store/state";
import { HuddleHeaderButton } from "../chat/Huddle";
import { HuddleControls, type HuddleControlsProps } from "./HuddleControls";
import { SelfCheck, type SelfCheckProps } from "./SelfCheck";

// ── WCAG plumbing (same shape as CallTiles.test.tsx's scrim gate) ──

/** Parse a color jsdom actually emits. A `var(...)` is NOT a color — it throws,
 *  which is the whole point: an inverting token can never satisfy this gate. */
const rgb = (css: string): number[] => {
  const s = css.trim();
  const short = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(s);
  if (short) return short.slice(1, 4).map((c) => Number.parseInt(c + c, 16) / 255);
  const long = /^#([0-9a-f]{6})$/i.exec(s);
  if (long) return [0, 2, 4].map((i) => Number.parseInt(long[1].slice(i, i + 2), 16) / 255);
  const fn = /^rgba?\(([^)]+)\)$/i.exec(s);
  if (fn) return fn[1].split(/[,\s/]+/).filter(Boolean).slice(0, 3).map((n) => Number(n) / 255);
  throw new Error(`not a concrete color: ${css}`);
};

const luminance = (c: number[]) => {
  const [r, g, b] = c.map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

const contrast = (a: number[], b: number[]) => {
  const [l1, l2] = [luminance(a), luminance(b)];
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
};

/** The literal `var(--accent, X)` falls back to — and, since nothing sets
 *  `--accent` per theme, the color actually painted for the default accent. */
const accentDefault = (bg: string) => {
  const m = /^var\(--accent,\s*(.+)\)$/.exec(bg.trim());
  if (!m) throw new Error(`expected the accent fill, got: ${bg}`);
  return m[1];
};

/** The rule, asserted on what the component really rendered. */
const readableOnAccent = (el: HTMLElement, what: string) => {
  const bg = el.style.background || el.style.backgroundColor;
  const fg = el.style.color;

  // Premise: this control is accent-filled. If that ever stops being true the
  // gate must fail loudly rather than pass vacuously on some other background.
  expect(bg, `${what}: expected the theme-invariant accent fill`).toMatch(/^var\(--accent,/);

  // The rule: an invariant background may not carry an inverting foreground.
  // Every `--c-*` flips with `data-theme`, so ANY var() here is the bug.
  expect(
    fg,
    `${what}: text on the accent must be a concrete, theme-invariant color (got ${fg || "nothing"})`,
  ).not.toMatch(/var\(/);
  expect(fg, `${what}: must set a text color`).not.toBe("");

  const ratio = contrast(rgb(fg), rgb(accentDefault(bg)));
  expect(ratio, `${what}: ${fg} on ${accentDefault(bg)} — WCAG AA`).toBeGreaterThanOrEqual(4.5);
  return ratio;
};

// ── The four sites ──

const selfCheck: SelfCheckProps = {
  status: "live",
  cameraOn: false,
  sharing: false,
  canEncode: true,
  muted: false,
  level: 0,
  speaking: false,
  bindPreview: vi.fn(),
  onToggleCamera: vi.fn(),
};

const controls: HuddleControlsProps = {
  size: "comfortable",
  status: "live",
  muted: false,
  cameraOn: false,
  canEncode: true,
  onToggleMute: vi.fn(),
  onToggleCamera: vi.fn(),
  onLeave: vi.fn(),
  onRetry: vi.fn(),
};

const channel: Channel = {
  id: "general",
  name: "general",
  created_at: 1,
  head_seq: 0,
  post_policy: "open",
  hooks: [],
  pinned: [],
  huddle: [],
};

/** Store state for "you are in this channel's huddle" — the header button's filled case. */
const inThisHuddle = (): ConsoleState => {
  const base = createInitialState();
  return {
    ...base,
    status: { version: "test", appHash: "0".repeat(64), height: 1, modules: [], publicKey: "ab".repeat(32) },
    voice: { ...base.voice, channelId: channel.id, status: "live" },
  };
};

describe("text on the theme-invariant accent stays readable in BOTH themes", () => {
  it("SelfCheck: the solo 'Turn on camera' prompt", () => {
    render(<SelfCheck {...selfCheck} />);
    const ratio = readableOnAccent(screen.getByRole("button", { name: /turn on camera/i }), "SelfCheck camera prompt");
    expect(ratio).toBeGreaterThanOrEqual(4.5);
  });

  it("HuddleControls: the camera button while the camera is on", () => {
    render(<HuddleControls {...controls} cameraOn />);
    readableOnAccent(screen.getByTitle("Turn camera off"), "HuddleControls camera-on");
  });

  it("HuddleControls: the screen button while sharing", () => {
    render(<HuddleControls {...controls} canScreenShare sharing onToggleScreen={vi.fn()} />);
    readableOnAccent(screen.getByTitle("Stop screen share"), "HuddleControls sharing");
  });

  it("HuddleHeaderButton: the channel header while you're in the huddle", () => {
    render(
      <ConsoleContext.Provider value={{ state: inThisHuddle(), actions: {} as unknown as ConsoleActions }}>
        <HuddleHeaderButton channel={channel} />
      </ConsoleContext.Provider>,
    );
    readableOnAccent(screen.getByTitle("Leave huddle"), "HuddleHeaderButton in-huddle");
  });
});
