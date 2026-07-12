// The node's Prometheus/OpenMetrics scrape, parsed into the `ducktape_*` block
// series the Metrics view charts. The raw text arrives over the node stream's
// `metrics` topic (one snapshot per heartbeat tick — the same exposition the
// node's GET /metrics serves scrapers); the wire names are fixed by the node
// (bin/noded NodeMetrics):
//
//   ducktape_block_height                       <gauge>
//   ducktape_blocks_total                       <counter>
//   ducktape_block_apply_latency_seconds_{sum,count,bucket{le="…"}}
//   ducktape_dispatch_total{module="…",origin="…"}   <counter family>
//
// plus the per-plane series (bin/node plane_metrics), every one labeled
// `{service, owner}` where `owner` is the module that created the plane —
// a plane that closes stops being scraped, so presence IS openness:
//
//   ducktape_dataplane_open / _halted / _age_seconds
//   ducktape_dataplane_bytes{dir="tx|rx",class="datagram|stream"}   (cumulative)
//   ducktape_dataplane_datagrams{dir="tx|rx"}                       (cumulative)
//   ducktape_dataplane_streams{kind="opened|accepted"}              (cumulative)
//   ducktape_dataplane_drops{kind="…"}                              (cumulative)
//
// plus the statesync SERVE lane (bin/node sync metrics) — statesync rides the
// mesh carrier, never a data plane, so it gets its own `{peer}`-labeled
// family. A peer that stops requesting ages out of the scrape — presence IS
// recent utilization:
//
//   ducktape_statesync_serve_age_seconds / _idle_seconds
//   ducktape_statesync_serve_bytes                                  (cumulative)
//   ducktape_statesync_serve_frames                                 (cumulative)
//   ducktape_statesync_serve_requests{kind="manifest|chunk|frames|…"} (cumulative)
//   ducktape_statesync_serve_boundary_height   (absent until a manifest is served)
//   ducktape_statesync_serve_frame_height      (absent until frames are served)
//
// A node serving only commonware's runtime series (an older binary that never
// records its own blocks) exposes none of these — `present` is then false.
// A node with no open planes (e.g. the embedded local daemon) simply has an
// empty `planes` list; a node no peer is syncing from has an empty `syncPeers`.

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

/** One open data plane, identified by `{service, owner}`. All byte/count
 *  fields are cumulative for the plane's life — derive rates from deltas. */
export interface DataPlaneMetric {
  service: string;
  /** the module that created the plane. */
  owner: string;
  ageSeconds: number;
  /** bound but no longer moving traffic (its pumps exited). */
  halted: boolean;
  /** wire bytes by direction and service class. */
  bytesTxDatagram: number;
  bytesRxDatagram: number;
  bytesTxStream: number;
  bytesRxStream: number;
  datagramsTx: number;
  datagramsRx: number;
  streamsOpened: number;
  streamsAccepted: number;
  /** dropped/refused traffic by kind (rogue_datagrams, shed, …). */
  drops: Record<string, number>;
}

/** One peer this node recently served state sync to, identified by `{peer}`
 *  (the requester's mesh key, hex). Byte/frame/request fields are cumulative
 *  for the peer's current sync conversation — derive rates from deltas. */
export interface StateSyncPeerMetric {
  /** the requesting peer's mesh public key, hex. */
  peer: string;
  ageSeconds: number;
  /** seconds since the peer's last answered request. */
  idleSeconds: number;
  /** wire bytes served to the peer. */
  bytesTx: number;
  /** finalized frames (blocks) served to the peer. */
  framesServed: number;
  /** the snapshot boundary height last served — the peer's restore base;
   *  null until a manifest is served (tip pollers never have one). */
  boundaryHeight: number | null;
  /** the highest frame (block) height served — the peer's replay reach;
   *  null until a frames batch is served. */
  servedHeight: number | null;
  /** answered requests by kind (manifest, chunk, frames, tip_coords, …). */
  requests: Record<string, number>;
}

export interface NodeMetrics {
  /** whether the node exposed its own `ducktape_*` block series at all. */
  present: boolean;
  blockHeight: number;
  blocksTotal: number;
  latency: LatencyHistogram;
  dispatches: DispatchCount[];
  /** every open data plane, sorted by service then owner. */
  planes: DataPlaneMetric[];
  /** every peer recently served over the statesync lane, sorted by peer. */
  syncPeers: StateSyncPeerMetric[];
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
  planes: [],
  syncPeers: [],
});

const emptyPlane = (service: string, owner: string): DataPlaneMetric => ({
  service,
  owner,
  ageSeconds: 0,
  halted: false,
  bytesTxDatagram: 0,
  bytesRxDatagram: 0,
  bytesTxStream: 0,
  bytesRxStream: 0,
  datagramsTx: 0,
  datagramsRx: 0,
  streamsOpened: 0,
  streamsAccepted: 0,
  drops: {},
});

const emptySyncPeer = (peer: string): StateSyncPeerMetric => ({
  peer,
  ageSeconds: 0,
  idleSeconds: 0,
  bytesTx: 0,
  framesServed: 0,
  boundaryHeight: null,
  servedHeight: null,
  requests: {},
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
  // planes keyed `{service} {owner}` while samples accumulate.
  const planes = new Map<string, DataPlaneMetric>();
  const planeOf = (labels: Record<string, string>): DataPlaneMetric => {
    const service = labels.service ?? "?";
    const owner = labels.owner ?? "?";
    const key = `${service} ${owner}`;
    const existing = planes.get(key);
    if (existing) return existing;
    const fresh = emptyPlane(service, owner);
    planes.set(key, fresh);
    return fresh;
  };
  // served peers keyed by the `{peer}` label while samples accumulate.
  const syncPeers = new Map<string, StateSyncPeerMetric>();
  const syncPeerOf = (labels: Record<string, string>): StateSyncPeerMetric => {
    const peer = labels.peer ?? "?";
    const existing = syncPeers.get(peer);
    if (existing) return existing;
    const fresh = emptySyncPeer(peer);
    syncPeers.set(peer, fresh);
    return fresh;
  };
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
      case "ducktape_dataplane_open":
        planeOf(s.labels);
        break;
      case "ducktape_dataplane_halted":
        planeOf(s.labels).halted = s.value > 0;
        break;
      case "ducktape_dataplane_age_seconds":
        planeOf(s.labels).ageSeconds = s.value;
        break;
      case "ducktape_dataplane_bytes": {
        const plane = planeOf(s.labels);
        const stream = s.labels.class === "stream";
        if (s.labels.dir === "tx") {
          if (stream) plane.bytesTxStream = s.value;
          else plane.bytesTxDatagram = s.value;
        } else if (stream) {
          plane.bytesRxStream = s.value;
        } else {
          plane.bytesRxDatagram = s.value;
        }
        break;
      }
      case "ducktape_dataplane_datagrams": {
        const plane = planeOf(s.labels);
        if (s.labels.dir === "tx") plane.datagramsTx = s.value;
        else plane.datagramsRx = s.value;
        break;
      }
      case "ducktape_dataplane_streams": {
        const plane = planeOf(s.labels);
        if (s.labels.kind === "opened") plane.streamsOpened = s.value;
        else plane.streamsAccepted = s.value;
        break;
      }
      case "ducktape_dataplane_drops":
        planeOf(s.labels).drops[s.labels.kind ?? "?"] = s.value;
        break;
      case "ducktape_statesync_serve_age_seconds":
        syncPeerOf(s.labels).ageSeconds = s.value;
        break;
      case "ducktape_statesync_serve_idle_seconds":
        syncPeerOf(s.labels).idleSeconds = s.value;
        break;
      case "ducktape_statesync_serve_bytes":
        syncPeerOf(s.labels).bytesTx = s.value;
        break;
      case "ducktape_statesync_serve_frames":
        syncPeerOf(s.labels).framesServed = s.value;
        break;
      case "ducktape_statesync_serve_requests":
        syncPeerOf(s.labels).requests[s.labels.kind ?? "?"] = s.value;
        break;
      case "ducktape_statesync_serve_boundary_height":
        syncPeerOf(s.labels).boundaryHeight = s.value;
        break;
      case "ducktape_statesync_serve_frame_height":
        syncPeerOf(s.labels).servedHeight = s.value;
        break;
      default:
        break; // runtime / other-module series are not charted here
    }
  }
  buckets.sort((a, b) => a.le - b.le);
  m.latency = { ...m.latency, buckets };
  m.planes = [...planes.values()].sort(
    (a, b) => a.service.localeCompare(b.service) || a.owner.localeCompare(b.owner),
  );
  m.syncPeers = [...syncPeers.values()].sort((a, b) => a.peer.localeCompare(b.peer));
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
 * A cumulative counter's per-second rate between two reads `dtMs` apart. A
 * counter that went DOWN (a node restart reset it) or a non-positive
 * interval reads as 0.
 */
export function ratePerSecond(prev: number, curr: number, dtMs: number): number {
  if (dtMs <= 0 || curr < prev) return 0;
  return ((curr - prev) * 1000) / dtMs;
}

/** Blocks/second between two `blocksTotal` reads — see [`ratePerSecond`]. */
export function blocksPerSecond(prev: number, curr: number, dtMs: number): number {
  return ratePerSecond(prev, curr, dtMs);
}

/** A plane's cumulative egress wire bytes, both service classes. */
export function planeTxBytes(p: DataPlaneMetric): number {
  return p.bytesTxDatagram + p.bytesTxStream;
}

/** A plane's cumulative ingress wire bytes, both service classes. */
export function planeRxBytes(p: DataPlaneMetric): number {
  return p.bytesRxDatagram + p.bytesRxStream;
}

/** A plane's cumulative dropped/refused traffic across every kind. */
export function planeDropTotal(p: DataPlaneMetric): number {
  return Object.values(p.drops).reduce((sum, n) => sum + n, 0);
}

/** The peer's block reach as this node served it: replayed frames if any,
 *  else the restored snapshot boundary. Null while nothing height-shaped has
 *  been served (a tip poller or blob fetcher). */
export function syncPeerReach(p: StateSyncPeerMetric): number | null {
  return p.servedHeight ?? p.boundaryHeight;
}

/** Blocks between the peer's reach and this node's own height (the goal
 *  block a syncing peer converges toward). Null before any reach exists. */
export function syncBlocksLeft(p: StateSyncPeerMetric, goalHeight: number): number | null {
  const reach = syncPeerReach(p);
  return reach === null ? null : Math.max(0, goalHeight - reach);
}

/** The peer's progression toward the goal block, 0..1. Null before any
 *  reach exists or while the goal is unknown (height 0). */
export function syncProgress(p: StateSyncPeerMetric, goalHeight: number): number | null {
  const reach = syncPeerReach(p);
  if (reach === null || goalHeight <= 0) return null;
  return Math.min(1, reach / goalHeight);
}

/** What the peer is currently doing on the lane, from its request mix —
 *  the phases of a join in order: manifest → snapshot chunks → frame replay;
 *  a peer doing none of these is polling coordinates or fetching blobs. */
export function syncPhase(p: StateSyncPeerMetric): string {
  const asked = (kind: string) => (p.requests[kind] ?? 0) > 0;
  // heterogeneous predicates per branch (not one discriminant), so if/else.
  if (p.servedHeight !== null) return "replaying frames";
  if (asked("chunk") || asked("module") || asked("index_chunk") || asked("index_modules")) {
    return "restoring snapshot";
  }
  if (p.boundaryHeight !== null) return "manifest served";
  if (asked("tip_coords")) return "polling tip";
  return "fetching blobs";
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

/** Bytes as a compact SI figure ("512 B", "1.35 kB", "24.0 MB"). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  const units = ["B", "kB", "MB", "GB", "TB"];
  const exp = Math.min(units.length - 1, bytes >= 1000 ? Math.floor(Math.log10(bytes) / 3) : 0);
  const v = bytes / 1000 ** exp;
  const digits = exp === 0 ? 0 : v < 10 ? 2 : v < 100 ? 1 : 0;
  return `${v.toFixed(digits)} ${units[exp]}`;
}

/** A byte rate as "1.35 kB/s". */
export function formatBytesRate(perSecond: number): string {
  return `${formatBytes(perSecond)}/s`;
}

/** A peer key's leading hex as a compact identity ("9f3ab2c1…"). */
export function formatPeer(peer: string): string {
  return peer.length > 8 ? `${peer.slice(0, 8)}…` : peer;
}

/** An age in seconds as the two most significant units ("2h 10m", "45s"). */
export function formatAge(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.floor(seconds);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  return `${Math.floor(s / 86400)}d ${Math.floor((s % 86400) / 3600)}h`;
}
