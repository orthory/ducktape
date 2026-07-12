import { describe, expect, it } from "vitest";

import {
  blocksPerSecond,
  formatAge,
  formatBound,
  formatBytes,
  formatBytesRate,
  formatLatency,
  formatRate,
  meanLatency,
  parseMetrics,
  perBucket,
  planeDropTotal,
  planeRxBytes,
  planeTxBytes,
  quantile,
  ratePerSecond,
} from "./metrics";

// A representative /metrics scrape: commonware runtime noise (must be ignored)
// followed by the node's ducktape_* block series in the exact wire shape the
// node emits (verified against a live daemon: counters carry ONE `_total`).
const SCRAPE = `# HELP chat_commit_calls Number of commit calls.
# TYPE chat_commit_calls counter
chat_commit_calls_total 4
# HELP runtime_tasks_spawned tasks
# TYPE runtime_tasks_spawned counter
runtime_tasks_spawned_total 17
# HELP ducktape_block_apply_latency_seconds node-local wall-clock cost of applying one block.
# TYPE ducktape_block_apply_latency_seconds histogram
ducktape_block_apply_latency_seconds_sum 0.02024
ducktape_block_apply_latency_seconds_count 2
ducktape_block_apply_latency_seconds_bucket{le="0.0001"} 0
ducktape_block_apply_latency_seconds_bucket{le="0.00025"} 0
ducktape_block_apply_latency_seconds_bucket{le="0.0005"} 0
ducktape_block_apply_latency_seconds_bucket{le="0.001"} 0
ducktape_block_apply_latency_seconds_bucket{le="0.0025"} 0
ducktape_block_apply_latency_seconds_bucket{le="0.005"} 0
ducktape_block_apply_latency_seconds_bucket{le="0.01"} 1
ducktape_block_apply_latency_seconds_bucket{le="0.025"} 2
ducktape_block_apply_latency_seconds_bucket{le="0.05"} 2
ducktape_block_apply_latency_seconds_bucket{le="0.1"} 2
ducktape_block_apply_latency_seconds_bucket{le="0.25"} 2
ducktape_block_apply_latency_seconds_bucket{le="0.5"} 2
ducktape_block_apply_latency_seconds_bucket{le="1.0"} 2
ducktape_block_apply_latency_seconds_bucket{le="+Inf"} 2
# HELP ducktape_block_height latest committed local block height.
# TYPE ducktape_block_height gauge
ducktape_block_height 2
# HELP ducktape_blocks_total committed local blocks since daemon start.
# TYPE ducktape_blocks_total counter
ducktape_blocks_total 2
# HELP ducktape_dispatch_total module dispatches, by module and trigger-origin kind.
# TYPE ducktape_dispatch_total counter
ducktape_dispatch_total{module="chat",origin="external"} 2
ducktape_dispatch_total{module="tagging",origin="module"} 1
# HELP ducktape_dataplane_open an open data plane, by service and creating module (1 = open).
# TYPE ducktape_dataplane_open gauge
ducktape_dataplane_open{service="voice",owner="chat"} 1
ducktape_dataplane_open{service="gateway",owner="gateway"} 1
# TYPE ducktape_dataplane_halted gauge
ducktape_dataplane_halted{service="voice",owner="chat"} 0
ducktape_dataplane_halted{service="gateway",owner="gateway"} 0
# TYPE ducktape_dataplane_age_seconds gauge
ducktape_dataplane_age_seconds{service="voice",owner="chat"} 90
ducktape_dataplane_age_seconds{service="gateway",owner="gateway"} 3725
# TYPE ducktape_dataplane_bytes gauge
ducktape_dataplane_bytes{service="voice",owner="chat",dir="tx",class="datagram"} 640000
ducktape_dataplane_bytes{service="voice",owner="chat",dir="rx",class="datagram"} 320000
ducktape_dataplane_bytes{service="voice",owner="chat",dir="tx",class="stream"} 0
ducktape_dataplane_bytes{service="voice",owner="chat",dir="rx",class="stream"} 0
ducktape_dataplane_bytes{service="gateway",owner="gateway",dir="tx",class="stream"} 150000
ducktape_dataplane_bytes{service="gateway",owner="gateway",dir="rx",class="stream"} 98000
# TYPE ducktape_dataplane_datagrams gauge
ducktape_dataplane_datagrams{service="voice",owner="chat",dir="tx"} 4000
ducktape_dataplane_datagrams{service="voice",owner="chat",dir="rx"} 2000
# TYPE ducktape_dataplane_streams gauge
ducktape_dataplane_streams{service="gateway",owner="gateway",kind="opened"} 12
ducktape_dataplane_streams{service="gateway",owner="gateway",kind="accepted"} 7
# TYPE ducktape_dataplane_drops gauge
ducktape_dataplane_drops{service="voice",owner="chat",kind="shed"} 5
ducktape_dataplane_drops{service="voice",owner="chat",kind="rogue_datagrams"} 2
ducktape_dataplane_drops{service="gateway",owner="gateway",kind="refused_sends"} 0
`;

describe("parseMetrics", () => {
  it("extracts the ducktape_* block series and ignores runtime noise", () => {
    const m = parseMetrics(SCRAPE);
    expect(m.present).toBe(true);
    expect(m.blockHeight).toBe(2);
    expect(m.blocksTotal).toBe(2);
    expect(m.latency.sum).toBeCloseTo(0.02024, 5);
    expect(m.latency.count).toBe(2);
    // 14 buckets including +Inf, ascending, last is Infinity.
    expect(m.latency.buckets).toHaveLength(14);
    expect(m.latency.buckets[0].le).toBe(0.0001);
    const lastBucket = m.latency.buckets[m.latency.buckets.length - 1];
    expect(lastBucket.le).toBe(Infinity);
    expect(lastBucket.cumulative).toBe(2);
    // dispatch family, both label rows, in order.
    expect(m.dispatches).toEqual([
      { module: "chat", origin: "external", count: 2 },
      { module: "tagging", origin: "module", count: 1 },
    ]);
  });

  it("assembles the per-plane families into open planes, sorted", () => {
    const m = parseMetrics(SCRAPE);
    expect(m.planes).toHaveLength(2);

    // sorted by service: gateway before voice.
    const [gateway, voice] = m.planes;
    expect(gateway.service).toBe("gateway");
    expect(gateway.owner).toBe("gateway");
    expect(gateway.ageSeconds).toBe(3725);
    expect(gateway.halted).toBe(false);
    expect(gateway.bytesTxStream).toBe(150000);
    expect(gateway.bytesRxStream).toBe(98000);
    expect(gateway.streamsOpened).toBe(12);
    expect(gateway.streamsAccepted).toBe(7);
    expect(gateway.drops).toEqual({ refused_sends: 0 });

    expect(voice.service).toBe("voice");
    expect(voice.owner).toBe("chat");
    expect(voice.bytesTxDatagram).toBe(640000);
    expect(voice.bytesRxDatagram).toBe(320000);
    expect(voice.datagramsTx).toBe(4000);
    expect(voice.datagramsRx).toBe(2000);
    expect(voice.drops).toEqual({ shed: 5, rogue_datagrams: 2 });
  });

  it("keeps `present` a block-series fact: plane series alone don't set it", () => {
    const m = parseMetrics(
      '# TYPE ducktape_dataplane_open gauge\nducktape_dataplane_open{service="voice",owner="chat"} 1\n',
    );
    expect(m.present).toBe(false);
    expect(m.planes).toHaveLength(1);
  });

  it("reads a node with only runtime series as not-present (an older binary)", () => {
    const m = parseMetrics(
      "# TYPE runtime_x counter\nruntime_x_total 3\nchat_commit_calls_total 1\n",
    );
    expect(m.present).toBe(false);
    expect(m.blocksTotal).toBe(0);
    expect(m.dispatches).toEqual([]);
    expect(m.planes).toEqual([]);
  });

  it("survives an empty / whitespace body", () => {
    expect(parseMetrics("").present).toBe(false);
    expect(parseMetrics("\n\n  \n").blocksTotal).toBe(0);
  });
});

describe("derivations", () => {
  const h = parseMetrics(SCRAPE).latency;

  it("de-cumulates buckets: two blocks landed in the 5–10ms and 10–25ms ranges", () => {
    const per = perBucket(h);
    expect(per.find((b) => b.le === 0.01)!.count).toBe(1); // (5ms, 10ms]
    expect(per.find((b) => b.le === 0.025)!.count).toBe(1); // (10ms, 25ms]
    expect(per.find((b) => b.le === Infinity)!.count).toBe(0);
    // the non-cumulative counts sum back to the total.
    expect(per.reduce((s, b) => s + b.count, 0)).toBe(h.count);
  });

  it("interpolates a quantile within the crossing bucket", () => {
    // rank for q=0.5 is 1.0; the 1st obs crosses at le=0.01 (prev le 0.005,
    // within-bucket count 1) → 0.005 + (0.005 * (1-0)/1) = 0.01.
    expect(quantile(h, 0.5)).toBeCloseTo(0.01, 6);
    // p99 rank 1.98 lands in (0.01, 0.025] → 0.01 + 0.015*(1.98-1)/1.
    expect(quantile(h, 0.99)).toBeCloseTo(0.01 + 0.015 * 0.98, 6);
    expect(quantile({ buckets: [], sum: 0, count: 0 }, 0.5)).toBeNull();
  });

  it("means latency as sum/count, null with no samples", () => {
    expect(meanLatency(h)).toBeCloseTo(0.01012, 5);
    expect(meanLatency({ buckets: [], sum: 0, count: 0 })).toBeNull();
  });

  it("rates a counter, guarding restarts and non-positive intervals", () => {
    expect(blocksPerSecond(10, 20, 2000)).toBe(5); // +10 over 2s
    expect(blocksPerSecond(20, 5, 1000)).toBe(0); // counter reset
    expect(blocksPerSecond(10, 20, 0)).toBe(0); // no elapsed time
    expect(ratePerSecond(0, 500_000, 2000)).toBe(250_000); // bytes ride the same math
  });

  it("totals a plane's directions and drops", () => {
    const [gateway, voice] = parseMetrics(SCRAPE).planes;
    expect(planeTxBytes(gateway)).toBe(150000);
    expect(planeRxBytes(gateway)).toBe(98000);
    expect(planeDropTotal(gateway)).toBe(0);
    expect(planeTxBytes(voice)).toBe(640000);
    expect(planeRxBytes(voice)).toBe(320000);
    expect(planeDropTotal(voice)).toBe(7);
  });
});

describe("formatting", () => {
  it("scales latency across µs / ms / s", () => {
    expect(formatLatency(0.00005)).toBe("50 µs");
    expect(formatLatency(0.0012)).toBe("1.20 ms");
    expect(formatLatency(0.042)).toBe("42.0 ms");
    expect(formatLatency(1.5)).toBe("1.50 s");
    expect(formatLatency(null)).toBe("—");
  });

  it("labels a bucket bound, with ∞ for the overflow", () => {
    expect(formatBound(0.0001)).toBe("100 µs");
    expect(formatBound(Infinity)).toBe("∞");
  });

  it("formats a rate", () => {
    expect(formatRate(4.2)).toBe("4.20 /s");
    expect(formatRate(42)).toBe("42.0 /s");
  });

  it("scales bytes across SI units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1350)).toBe("1.35 kB");
    expect(formatBytes(24_000_000)).toBe("24.0 MB");
    expect(formatBytes(3_200_000_000)).toBe("3.20 GB");
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytesRate(1350)).toBe("1.35 kB/s");
  });

  it("ages in the two most significant units", () => {
    expect(formatAge(45)).toBe("45s");
    expect(formatAge(90)).toBe("1m 30s");
    expect(formatAge(3725)).toBe("1h 2m");
    expect(formatAge(2 * 86400 + 3 * 3600)).toBe("2d 3h");
    expect(formatAge(-1)).toBe("—");
  });
});
