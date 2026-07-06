// Dependency-free inline-SVG chart primitives for the Metrics dashboard. Each
// is a single-series magnitude/time mark, so color is one accent (the app's
// --accent) and all text wears ink tokens — no categorical palette, no legend.
// Marks follow the house spec: thin bars with 4px rounded data-ends anchored to
// the baseline, a 2px surface gap between fills, 2px lines, recessive axes.

import type { CSSProperties, ReactNode } from "react";

import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const labelStyle: CSSProperties = {
  font: `600 9.5px ${font.mono}`,
  letterSpacing: ".08em",
  color: color.muted2,
  textTransform: "uppercase",
};

/** A headline number with a caption and optional sub-line. */
export function StatTile({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: ReactNode;
}) {
  return (
    <div
      style={{
        flex: "1 1 130px",
        minWidth: 0,
        display: "flex",
        flexDirection: "column",
        gap: 5,
        padding: "12px 14px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
      }}
    >
      <span style={labelStyle}>{label}</span>
      <span
        style={{
          font: `600 22px ${font.mono}`,
          color: color.ink,
          lineHeight: 1,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {value}
      </span>
      {sub !== undefined && (
        <span style={{ font: `400 11px ${font.mono}`, color: color.muted }}>{sub}</span>
      )}
    </div>
  );
}

/** A titled panel wrapper the charts sit in. */
export function Panel({
  title,
  right,
  children,
  grow = 1,
}: {
  title: string;
  right?: ReactNode;
  children: ReactNode;
  grow?: number;
}) {
  return (
    <div
      style={{
        flex: `${grow} 1 260px`,
        minWidth: 0,
        display: "flex",
        flexDirection: "column",
        gap: 12,
        padding: "13px 15px 15px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
      }}
    >
      <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between" }}>
        <span style={{ font: `600 12px ${font.sans}`, color: color.ink }}>{title}</span>
        {right !== undefined && (
          <span style={{ font: `500 11px ${font.mono}`, color: color.muted }}>{right}</span>
        )}
      </div>
      {children}
    </div>
  );
}

/** A 2px line over a number series, with a soft area fill and the last point
 *  labeled. Flat (or empty) series render a baseline. */
export function Sparkline({
  values,
  height = 64,
  format,
}: {
  values: number[];
  height?: number;
  format: (v: number) => string;
}) {
  const w = 100; // viewBox width; the svg scales to its container
  const pad = 3;
  const max = Math.max(1e-9, ...values);
  const n = values.length;
  const x = (i: number) => (n <= 1 ? w : pad + (i * (w - 2 * pad)) / (n - 1));
  const y = (v: number) => height - pad - (Math.max(0, v) / max) * (height - 2 * pad);
  const pts = values.map((v, i) => `${x(i)},${y(v)}`);
  const last = values.length ? values[values.length - 1] : 0;

  return (
    <div style={{ position: "relative" }}>
      <svg
        viewBox={`0 0 ${w} ${height}`}
        preserveAspectRatio="none"
        style={{ width: "100%", height, display: "block" }}
        role="img"
        aria-label={`series, latest ${format(last)}`}
      >
        <line
          x1={0}
          y1={height - pad}
          x2={w}
          y2={height - pad}
          stroke={color.borderSoft}
          strokeWidth={1}
          vectorEffect="non-scaling-stroke"
        />
        {n >= 2 && (
          <>
            <polygon
              points={`${x(0)},${height - pad} ${pts.join(" ")} ${x(n - 1)},${height - pad}`}
              fill={accentVar}
              opacity={0.1}
            />
            <polyline
              points={pts.join(" ")}
              fill="none"
              stroke={accentVar}
              strokeWidth={2}
              strokeLinejoin="round"
              strokeLinecap="round"
              vectorEffect="non-scaling-stroke"
            />
          </>
        )}
        {n >= 1 && (
          <circle cx={x(n - 1)} cy={y(last)} r={2.5} fill={accentVar} vectorEffect="non-scaling-stroke" />
        )}
      </svg>
    </div>
  );
}

/** A vertical histogram of per-bucket counts, with optional quantile markers
 *  (thin lines at the bucket a quantile falls in). Bars are one accent hue. */
export function Histogram({
  bars,
  markers = [],
  boundLabel,
}: {
  bars: { key: string; count: number; hint: string }[];
  markers?: { at: number; label: string }[]; // `at` = bucket index
  boundLabel: (i: number) => string | null;
}) {
  const height = 96;
  const max = Math.max(1, ...bars.map((b) => b.count));
  const n = Math.max(1, bars.length);
  const slot = 100 / n;
  const barW = slot * 0.62;
  const gap = (slot - barW) / 2;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <svg
        viewBox={`0 0 100 ${height}`}
        preserveAspectRatio="none"
        style={{ width: "100%", height, display: "block" }}
        role="img"
        aria-label="apply-latency distribution by bucket"
      >
        {/* baseline */}
        <line
          x1={0}
          y1={height}
          x2={100}
          y2={height}
          stroke={color.borderSoft}
          strokeWidth={1}
          vectorEffect="non-scaling-stroke"
        />
        {bars.map((b, i) => {
          const h = (b.count / max) * (height - 6);
          const barX = i * slot + gap;
          return (
            <g key={b.key}>
              <title>{b.hint}</title>
              {/* full-slot invisible hit target for the tooltip */}
              <rect x={i * slot} y={0} width={slot} height={height} fill="transparent" />
              <rect
                x={barX}
                y={height - Math.max(h, b.count > 0 ? 2 : 0)}
                width={barW}
                height={Math.max(h, b.count > 0 ? 2 : 0)}
                rx={Math.min(2, barW / 2)}
                fill={accentVar}
                opacity={b.count > 0 ? 0.9 : 0}
              />
            </g>
          );
        })}
        {markers.map((mk) => {
          const mx = mk.at * slot + slot / 2;
          return (
            <line
              key={mk.label}
              x1={mx}
              y1={2}
              x2={mx}
              y2={height}
              stroke={color.ink}
              strokeWidth={1}
              strokeDasharray="2 2"
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
      </svg>
      {/* sparse x-axis labels + marker captions */}
      <div style={{ position: "relative", height: 12 }}>
        {bars.map((_, i) => {
          const text = boundLabel(i);
          if (!text) return null;
          return (
            <span
              key={i}
              style={{
                position: "absolute",
                left: `${((i + 0.5) / n) * 100}%`,
                transform: "translateX(-50%)",
                font: `400 8.5px ${font.mono}`,
                color: color.muted2,
                whiteSpace: "nowrap",
              }}
            >
              {text}
            </span>
          );
        })}
      </div>
      {markers.length > 0 && (
        <div style={{ display: "flex", gap: 14, flexWrap: "wrap" }}>
          {markers.map((mk) => (
            <span key={mk.label} style={{ font: `500 10.5px ${font.mono}`, color: color.muted3 }}>
              {mk.label}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

/** Horizontal ranked bars: one accent hue, each row direct-labeled. */
export function RankedBars({
  rows,
}: {
  rows: { key: string; label: string; value: number; valueLabel: string }[];
}) {
  const max = Math.max(1, ...rows.map((r) => r.value));
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
      {rows.map((r) => (
        <div key={r.key} style={{ display: "flex", alignItems: "center", gap: 9 }}>
          <span
            style={{
              font: `500 11.5px ${font.sans}`,
              color: color.ink,
              width: 96,
              flexShrink: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
            title={r.label}
          >
            {r.label}
          </span>
          <div style={{ flex: 1, minWidth: 0, height: 12, position: "relative" }}>
            <div
              style={{
                position: "absolute",
                inset: 0,
                borderRadius: radius.sm,
                background: color.sunken,
              }}
            />
            <div
              style={{
                position: "absolute",
                left: 0,
                top: 0,
                bottom: 0,
                width: `${Math.max((r.value / max) * 100, r.value > 0 ? 3 : 0)}%`,
                borderRadius: radius.sm,
                background: accentVar,
                opacity: 0.9,
              }}
            />
          </div>
          <span
            style={{
              font: `500 11px ${font.mono}`,
              color: color.muted3,
              width: 44,
              textAlign: "right",
              flexShrink: 0,
            }}
          >
            {r.valueLabel}
          </span>
        </div>
      ))}
    </div>
  );
}
