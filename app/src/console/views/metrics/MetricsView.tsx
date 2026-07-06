// The metrics surface: the node's own operational health, scraped from
// GET /metrics (Prometheus) and read live. Unlike the Explorer (WHAT the chain
// did — durable, canonical), this is HOW THIS node is running: block height and
// throughput, apply-latency distribution, and which modules are busiest. Poll
// only while mounted; a node that doesn't report the `ducktape_*` block series
// (an older binary) says so plainly rather than drawing empty charts.

import { useEffect, useMemo, useState } from "react";

import {
  blocksPerSecond,
  formatBound,
  formatLatency,
  formatRate,
  meanLatency,
  perBucket,
  quantile,
  type NodeMetrics,
} from "../../../domain/metrics";
import { useDucktape } from "../../store/use-ducktape";
import { color, font } from "../../theme/tokens";
import { Histogram, Panel, RankedBars, Sparkline, StatTile } from "./charts";

/** How often to re-scrape, and how many samples the rolling window keeps. */
const POLL_MS = 2_000;
const WINDOW = 90; // ~3 min at 2s

interface Sample {
  t: number;
  m: NodeMetrics;
}

const emptyStyle = { font: `400 12px ${font.sans}`, color: color.muted2 } as const;

export function MetricsView() {
  const { state, actions } = useDucktape();
  const { connected, nodeUrl } = state;
  const [samples, setSamples] = useState<Sample[]>([]);

  // Scrape /metrics on an interval while mounted + connected. Reset the window
  // when the node changes so one node's samples never bleed into another's.
  useEffect(() => {
    setSamples([]);
    if (!connected) return;
    let cancelled = false;
    const poll = () => {
      actions.readMetrics().then((m) => {
        if (cancelled || !m) return;
        setSamples((prev) => [...prev, { t: Date.now(), m }].slice(-WINDOW));
      });
    };
    poll();
    const timer = setInterval(poll, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [connected, nodeUrl, actions]);

  const latest = samples.length ? samples[samples.length - 1].m : undefined;

  // blocks/second across the rolling window (counter deltas over wall time).
  const throughput = useMemo(() => {
    const series: number[] = [];
    for (let i = 1; i < samples.length; i++) {
      const a = samples[i - 1];
      const b = samples[i];
      series.push(blocksPerSecond(a.m.blocksTotal, b.m.blocksTotal, b.t - a.t));
    }
    return series;
  }, [samples]);

  // dispatch counters summed per module (across origin kinds), busiest first.
  const modules = useMemo(() => {
    if (!latest) return [];
    const total = new Map<string, number>();
    for (const d of latest.dispatches) {
      total.set(d.module, (total.get(d.module) ?? 0) + d.count);
    }
    return [...total.entries()]
      .map(([module, count]) => ({ module, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, 8);
  }, [latest]);

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "11px 17px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Metrics</span>
        {connected && latest?.present && (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              font: `500 11px ${font.mono}`,
              color: color.muted,
            }}
          >
            <span
              style={{
                width: 6,
                height: 6,
                borderRadius: 3,
                background: color.green,
                boxShadow: `0 0 0 2px ${color.paper}`,
              }}
            />
            live
          </span>
        )}
      </div>

      {!connected ? (
        <div style={{ padding: 17 }}>
          <div style={emptyStyle}>
            Not connected — metrics stream from the node's <code>/metrics</code> scrape once
            it's reachable.
          </div>
        </div>
      ) : !latest ? (
        <div style={{ padding: 17 }}>
          <div style={emptyStyle}>Reading /metrics…</div>
        </div>
      ) : !latest.present ? (
        <div style={{ padding: 17 }}>
          <div style={emptyStyle}>
            This node isn't reporting block metrics — its <code>/metrics</code> carries only the
            runtime series. Rebuild and restart it (<code>make dev</code>) and the{" "}
            <code>ducktape_*</code> series appear here as it commits blocks.
          </div>
        </div>
      ) : (
        <MetricsBody latest={latest} throughput={throughput} modules={modules} />
      )}
    </div>
  );
}

function MetricsBody({
  latest,
  throughput,
  modules,
}: {
  latest: NodeMetrics;
  throughput: number[];
  modules: { module: string; count: number }[];
}) {
  const hist = latest.latency;
  const per = perBucket(hist);
  const mean = meanLatency(hist);
  const p50 = quantile(hist, 0.5);
  const p99 = quantile(hist, 0.99);
  const currentTp = throughput.length ? throughput[throughput.length - 1] : 0;

  // sparse x-axis labels: first, last, and a few between (avoid collisions).
  const labelEvery = Math.max(1, Math.ceil(per.length / 5));
  const boundLabel = (i: number): string | null =>
    i === per.length - 1 || i % labelEvery === 0 ? formatBound(per[i].le) : null;

  // place a quantile's marker at the bucket its value falls into.
  const markerAt = (q: number | null): number | null => {
    if (q === null) return null;
    const idx = hist.buckets.findIndex((b) => b.le >= q);
    return idx < 0 ? hist.buckets.length - 1 : idx;
  };
  const markers = [
    { at: markerAt(p50), label: `p50 ${formatLatency(p50)}` },
    { at: markerAt(p99), label: `p99 ${formatLatency(p99)}` },
  ].filter((mk): mk is { at: number; label: string } => mk.at !== null);

  const bars = per.map((b, i) => {
    const lo = i === 0 ? 0 : per[i - 1].le;
    return {
      key: String(b.le),
      count: b.count,
      hint: `${b.count.toLocaleString()} block${b.count === 1 ? "" : "s"} in ${formatBound(lo)}–${formatBound(b.le)}`,
    };
  });

  return (
    <div style={{ padding: 17, display: "flex", flexDirection: "column", gap: 13, overflowY: "auto" }}>
      {/* KPI row */}
      <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
        <StatTile label="Height" value={latest.blockHeight.toLocaleString()} />
        <StatTile label="Blocks" value={latest.blocksTotal.toLocaleString()} sub="since start" />
        <StatTile label="Rate" value={formatRate(currentTp)} sub="blocks / sec" />
        <StatTile
          label="Latency"
          value={formatLatency(mean)}
          sub={p99 !== null ? `p99 ${formatLatency(p99)}` : "mean"}
        />
      </div>

      {/* distribution + throughput */}
      <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
        <Panel title="Apply latency" right={`${hist.count.toLocaleString()} samples`} grow={1.7}>
          {hist.count > 0 ? (
            <Histogram bars={bars} markers={markers} boundLabel={boundLabel} />
          ) : (
            <div style={emptyStyle}>No blocks applied yet.</div>
          )}
        </Panel>
        <Panel title="Throughput" right={formatRate(currentTp)} grow={1}>
          <Sparkline values={throughput} format={formatRate} />
          <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>
            blocks / sec, last {Math.max(0, throughput.length)} samples
          </span>
        </Panel>
      </div>

      {/* dispatches by module */}
      <Panel title="Dispatches by module" right="since start">
        {modules.length > 0 ? (
          <RankedBars
            rows={modules.map((r) => ({
              key: r.module,
              label: r.module,
              value: r.count,
              valueLabel: r.count.toLocaleString(),
            }))}
          />
        ) : (
          <div style={emptyStyle}>No dispatches yet.</div>
        )}
      </Panel>
    </div>
  );
}
