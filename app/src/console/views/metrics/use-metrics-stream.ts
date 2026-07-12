// Live node metrics over the shared /v1/ws stream — the `metrics` topic
// replaced the per-view GET /metrics polling loops. The node pushes one whole
// OpenMetrics snapshot on subscribe and one per heartbeat tick (~3 s); each
// sample parses client-side (domain/metrics) into the rolling window that
// rate derivations (blocks/sec, plane throughput) delta over. Samples carry
// the SERVER's sample instant, so rates never absorb frame-arrival jitter.
// The transport refcounts topics: the Metrics dashboard and the Node overview
// mounting together still cost one subscription on the one shared socket, and
// the last unmount drops it — the node samples only while someone watches.

import { useEffect, useMemo, useState } from "react";

import { parseMetrics, type NodeMetrics } from "../../../domain/metrics";
import { METRICS_TOPIC, isMetricsTailItem } from "../../../domain/stream";
import type { NodeTransport } from "../../../domain/transport";

// ── Types ───────────────────────────────────────────────

/** How many samples the rolling window keeps (~4.5 min at the 3 s tick). */
const WINDOW = 90;

export interface MetricsSample {
  /** the server-side sample instant, unix ms. */
  t: number;
  m: NodeMetrics;
}

export interface MetricsStream {
  /** the rolling window, oldest first; empty until the first sample lands. */
  samples: MetricsSample[];
  latest: NodeMetrics | null;
  /** the node refused the topic (an older daemon build) — no samples will
   *  ever arrive on this connection, so consumers can say so honestly. */
  refused: boolean;
}

// ── Hook ────────────────────────────────────────────────

export function useMetricsStream(
  transport: NodeTransport | null | undefined,
  connected: boolean,
): MetricsStream {
  const [samples, setSamples] = useState<MetricsSample[]>([]);
  const [refused, setRefused] = useState(false);

  // Reset the window when the transport (the node) changes or the stream
  // drops, so one node's samples never bleed into another's.
  useEffect(() => {
    setSamples([]);
    setRefused(false);
    if (!transport || !connected) return;
    return transport.subscribe([METRICS_TOPIC], {
      onTail: (frame) => {
        const item = frame.item;
        if (frame.topic !== METRICS_TOPIC || !isMetricsTailItem(item)) return;
        const sample = { t: item.timeMs, m: parseMetrics(item.text) };
        setSamples((prev) => [...prev, sample].slice(-WINDOW));
      },
      onRefused: (topic) => {
        if (topic === METRICS_TOPIC) setRefused(true);
      },
    });
  }, [transport, connected]);

  return useMemo(
    () => ({
      samples,
      latest: samples.length > 0 ? samples[samples.length - 1].m : null,
      refused,
    }),
    [samples, refused],
  );
}
