// The status-page health bar — a fixed grid of thin ticks, newest on the
// right, that reads at a glance like `||||||||:|`. Each tick is one finalized
// block: a full green tick applied cleanly, a short amber tick finalized-but-
// rejected (a failed tx — texture, not a node fault), and faint stub ticks pad
// the left when the ring holds fewer blocks than slots. The newest tick pulses
// while the node is live. Pure presentation over `HealthSeg[]` — no fetching.

import type { CSSProperties } from "react";

import { color, font, radius } from "../../theme/tokens";
import type { HealthSeg } from "./node-health";

const APPLIED = "#5f9e74"; // the synced-green used by the status pill
const REJECTED = color.amber;

const BAR_HEIGHT = 30;
const H_APPLIED = 1; // fraction of BAR_HEIGHT per kind — the `|` vs `:` shape
const H_REJECTED = 0.42;
const H_EMPTY = 0.24;

type Kind = "applied" | "rejected" | "empty";

const tickColor = (kind: Kind): string =>
  kind === "applied" ? APPLIED : kind === "rejected" ? REJECTED : color.borderStrong;

const tickHeight = (kind: Kind): number =>
  BAR_HEIGHT *
  (kind === "applied" ? H_APPLIED : kind === "rejected" ? H_REJECTED : H_EMPTY);

function Tick({ kind, title, live }: { kind: Kind; title?: string; live?: boolean }) {
  const style: CSSProperties = {
    flex: "1 1 0",
    minWidth: 2,
    maxWidth: 7,
    height: tickHeight(kind),
    borderRadius: 2,
    background: tickColor(kind),
    opacity: kind === "empty" ? 0.5 : 1,
    alignSelf: "flex-end",
  };
  if (live) style.animation = "ik-pulse 1.6s ease-in-out infinite";
  return <div title={title} style={style} />;
}

export function HealthBar({
  segments,
  slots = 48,
  live = false,
}: {
  /** Oldest-first; the last entry is the newest block (renders rightmost). */
  segments: readonly HealthSeg[];
  /** Total tick columns; missing history pads the left with faint stubs. */
  slots?: number;
  /** Pulse the newest tick — the node is following the chain. */
  live?: boolean;
}) {
  const shown = segments.slice(-slots);
  const pad = Math.max(0, slots - shown.length);
  const lastIndex = shown.length - 1;
  const rejected = shown.reduce((n, s) => n + (s.disposition === "rejected" ? 1 : 0), 0);

  return (
    <div
      role="img"
      aria-label={
        shown.length === 0
          ? "No recent commits"
          : `${shown.length} recent commits, ${rejected} rejected`
      }
      style={{
        display: "flex",
        alignItems: "flex-end",
        gap: 2,
        height: BAR_HEIGHT,
      }}
    >
      {Array.from({ length: pad }, (_, i) => (
        <Tick key={`pad-${i}`} kind="empty" />
      ))}
      {shown.map((seg, i) => (
        <Tick
          key={seg.height}
          kind={seg.disposition === "rejected" ? "rejected" : "applied"}
          title={`#${seg.height.toLocaleString()} · ${seg.disposition}`}
          live={live && i === lastIndex}
        />
      ))}
    </div>
  );
}

/** The bar's legend + tallies, sized to sit under a HealthBar. */
export function HealthLegend({
  applied,
  rejected,
  span,
}: {
  applied: number;
  rejected: number;
  /** e.g. "#1,240 – #1,288" — the height range the strip covers. */
  span: string | null;
}) {
  const swatch = (c: string) => ({
    width: 8,
    height: 8,
    borderRadius: 2,
    background: c,
    flexShrink: 0,
  });
  const item = { display: "inline-flex", alignItems: "center", gap: 5 } as const;
  const text = { font: `500 10.5px ${font.mono}`, color: color.muted2 } as const;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 14,
        flexWrap: "wrap",
      }}
    >
      <span style={item}>
        <span style={swatch(APPLIED)} />
        <span style={text}>{applied.toLocaleString()} applied</span>
      </span>
      <span style={item}>
        <span style={swatch(REJECTED)} />
        <span style={text}>{rejected.toLocaleString()} rejected</span>
      </span>
      {span && (
        <span style={{ ...text, marginLeft: "auto", fontVariant: "tabular-nums" as const }}>
          {span}
        </span>
      )}
      <span
        style={{
          font: `500 10.5px ${font.mono}`,
          color: color.muted2,
          border: `1px solid ${color.borderSoft}`,
          borderRadius: radius.sm,
          padding: "1px 6px",
          marginLeft: span ? 0 : "auto",
        }}
      >
        non-empty blocks
      </span>
    </div>
  );
}
