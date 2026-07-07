// The node's Prometheus/OpenMetrics scrape, parsed into the `ducktape_*` block
// series the Metrics view charts. See `NodeTransport.metrics()` for the raw
// text; the wire names are fixed by the node (bin/noded NodeMetrics):
//
//   ducktape_block_height                       <gauge>
//   ducktape_blocks_total                       <counter>
//   ducktape_block_apply_latency_seconds_{sum,count,bucket{le="…"}}
//   ducktape_dispatch_total{module="…",origin="…"}   <counter family>
//
// A node serving only commonware's runtime series (an older binary that never
// records its own blocks) exposes none of these — `present` is then false.

/** One cumulative histogram bucket: `cumulative` observations were ≤ `le` s. */
export interface Bucket {
  /** upper bound in seconds; the `+Inf` overflow bucket is `Infinity`. */
  le: number;
  cumulative: number;
}

export interface LatencyHistogram {
  /** cumulative buckets, ascending by `le` (last is `Infinity`). */
  buckets: Bucket[];
  /** total observed apply time, seconds. */
  sum: number;
  /** total blocks observed. */
  count: number;
}

export interface DispatchCount {
  module: string;
  /** the trigger KIND: "external" | "module" | "system". */
  origin: string;
  count: number;
}

export interface NodeMetrics {
  /** whether the node exposed its own `ducktape_*` block series at all. */
  present: boolean;
  blockHeight: number;
  blocksTotal: number;
  latency: LatencyHistogram;
  dispatches: DispatchCount[];
}

/** One parsed exposition sample: `name{labels} value`. */
interface Sample {
  name: string;
  labels: Record<string, string>;
  value: number;
}

const EMPTY_HISTOGRAM: LatencyHistogram = { buckets: [], sum: 0, count: 0 };

export const emptyMetrics = (): NodeMetrics => ({
  present: false,
  blockHeight: 0,
  blocksTotal: 0,
  latency: EMPTY_HISTOGRAM,
  dispatches: [],
});

/** `{module="chat",origin="external"}` → `{ module: "chat", origin: "external" }`. */
const parseLabels = (raw: string): Record<string, string> => {
  const labels: Record<string, string> = {};
  // key="value" pairs; values may contain commas/spaces, so match quoted spans.
  for (const [, key, value] of raw.matchAll(/([a-zA-Z_][\w]*)="([^"]*)"/g)) {
    labels[key] = value;
  }
  return labels;
};

/** Parse one non-comment exposition line into a `Sample`, or null if malformed. */
const parseLine = (line: string): Sample | null => {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith("#")) return null;
  const brace = trimmed.indexOf("{");
  let name: string;
  let labels: Record<string, string>;
  let rest: string;
  if (brace >= 0) {
    const close = trimmed.indexOf("}", brace);
    if (close < 0) return null;
    name = trimmed.slice(0, brace);
    labels = parseLabels(trimmed.slice(brace + 1, close));
    rest = trimmed.slice(close + 1);
  } else {
    const sp = trimmed.indexOf(" ");
    if (sp < 0) return null;
    name = trimmed.slice(0, sp);
    labels = {};
    rest = trimmed.slice(sp + 1);
  }
  // the value is the first whitespace token after the name/labels (a trailing
  // OpenMetrics timestamp/exemplar, if any, is ignored).
  const token = rest.trim().split(/\s+/)[0];
  const value = token === "+Inf" ? Infinity : Number(token);
  if (!Number.isFinite(value) && token !== "+Inf") return null;
  return { name, labels, value };
};

/** Parse a full OpenMetrics exposition into the `ducktape_*` block series. */
export function parseMetrics(text: string): NodeMetrics {
  const m = emptyMetrics();
  const buckets: Bucket[] = [];
  for (const line of text.split("\n")) {
    const s = parseLine(line);
    if (!s) continue;
    switch (s.name) {
      case "ducktape_block_height":
        m.present = true;
        m.blockHeight = s.value;
        break;
      case "ducktape_blocks_total":
        m.present = true;
        m.blocksTotal = s.value;
        break;
      case "ducktape_block_apply_latency_seconds_sum":
        m.present = true;
        m.latency = { ...m.latency, sum: s.value };
        break;
      case "ducktape_block_apply_latency_seconds_count":
        m.present = true;
        m.latency = { ...m.latency, count: s.value };
        break;
      case "ducktape_block_apply_latency_seconds_bucket": {
        m.present = true;
        const le = s.labels.le === "+Inf" ? Infinity : Number(s.labels.le);
        if (Number.isFinite(le) || le === Infinity) {
          buckets.push({ le, cumulative: s.value });
        }
        break;
      }
      case "ducktape_dispatch_total":
        m.present = true;
        m.dispatches.push({
          module: s.labels.module ?? "?",
          origin: s.labels.origin ?? "?",
          count: s.value,
        });
        break;
      default:
        break; // runtime / other-module series are not charted here
    }
  }
  buckets.sort((a, b) => a.le - b.le);
  m.latency = { ...m.latency, buckets };
  return m;
}

// ── Derivations ─────────────────────────────────────────

/** Non-cumulative count in each bucket range `(prevLe, le]`, ascending. */
export function perBucket(h: LatencyHistogram): { le: number; count: number }[] {
  let prev = 0;
  return h.buckets.map((b) => {
    const count = Math.max(0, b.cumulative - prev);
    prev = b.cumulative;
    return { le: b.le, count };
  });
}

/**
 * The `q`-quantile (0..1) apply latency in seconds, by the standard histogram
 * method: find the bucket the rank falls in and linearly interpolate within it
 * (lower bound 0 for the first bucket). Returns null when there are no samples.
 * The `+Inf` bucket has no finite upper bound, so a quantile that lands there
 * clamps to the largest finite boundary.
 */
export function quantile(h: LatencyHistogram, q: number): number | null {
  if (h.count <= 0 || h.buckets.length === 0) return null;
  const rank = q * h.count;
  let prevLe = 0;
  let prevCum = 0;
  for (const b of h.buckets) {
    if (b.cumulative >= rank) {
      if (b.le === Infinity) return prevLe; // clamp to the last finite boundary
      const span = b.le - prevLe;
      const within = b.cumulative - prevCum;
      if (within <= 0) return prevLe;
      return prevLe + (span * (rank - prevCum)) / within;
    }
    prevLe = b.le === Infinity ? prevLe : b.le;
    prevCum = b.cumulative;
  }
  return prevLe;
}

/** Mean apply latency in seconds (`sum / count`), or null with no samples. */
export function meanLatency(h: LatencyHistogram): number | null {
  return h.count > 0 ? h.sum / h.count : null;
}

/**
 * Blocks/second between two counter reads `dtMs` apart. A counter that went
 * DOWN (a node restart reset it) or a non-positive interval reads as 0.
 */
export function blocksPerSecond(prev: number, curr: number, dtMs: number): number {
  if (dtMs <= 0 || curr < prev) return 0;
  return ((curr - prev) * 1000) / dtMs;
}

// ── Formatting ──────────────────────────────────────────

/** Seconds → a compact, legible latency (µs / ms / s). */
export function formatLatency(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return "—";
  const us = seconds * 1_000_000;
  if (us < 1000) return `${Math.round(us)} µs`;
  if (us < 1_000_000) return `${(us / 1000).toFixed(us < 10_000 ? 2 : 1)} ms`;
  return `${seconds.toFixed(2)} s`;
}

/** A bucket's `le` bound as an axis label ("100 µs", "1 ms", "∞"). */
export function formatBound(le: number): string {
  return le === Infinity ? "∞" : formatLatency(le);
}

/** A rate as a compact "N.N /s". */
export function formatRate(perSecond: number): string {
  return `${perSecond.toFixed(perSecond < 10 ? 2 : 1)} /s`;
}
