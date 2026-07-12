// The Metrics view subscribes to the node stream's `metrics` topic while
// mounted and charts the ducktape_* series each pushed snapshot carries — with
// honest empty states for disconnected, for an older daemon that refuses the
// topic, and for a node that reports only runtime metrics (an older binary
// whose scrape has no block series).

import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { METRICS_TOPIC } from "../../../domain/stream";
import type { NodeTransport, TopicHandlers } from "../../../domain/transport";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { MetricsView } from "./MetricsView";

// ── Exposition fixtures (what the stream's tail frames carry) ──

const runtimeOnlyText = ["runtime_tasks_running 3", "# EOF", ""].join("\n");

const withDataText = [
  "# TYPE ducktape_block_height gauge",
  "runtime_tasks_running 3", // runtime noise the parser must ignore
  "ducktape_block_height 42",
  "ducktape_blocks_total 100",
  "ducktape_block_apply_latency_seconds_sum 0.03",
  "ducktape_block_apply_latency_seconds_count 3",
  'ducktape_block_apply_latency_seconds_bucket{le="0.001"} 0',
  'ducktape_block_apply_latency_seconds_bucket{le="0.01"} 2',
  'ducktape_block_apply_latency_seconds_bucket{le="0.1"} 3',
  'ducktape_block_apply_latency_seconds_bucket{le="+Inf"} 3',
  'ducktape_dispatch_total{module="chat",origin="external"} 5',
  'ducktape_dispatch_total{module="chat",origin="module"} 2',
  'ducktape_dispatch_total{module="tagging",origin="module"} 3',
  "# EOF",
  "",
].join("\n");

const withPlanesText = [
  withDataText.replace("# EOF\n", ""),
  'ducktape_dataplane_open{service="voice",owner="chat"} 1',
  'ducktape_dataplane_age_seconds{service="voice",owner="chat"} 90',
  'ducktape_dataplane_bytes{service="voice",owner="chat",dir="tx",class="datagram"} 640000',
  'ducktape_dataplane_bytes{service="voice",owner="chat",dir="rx",class="datagram"} 320000',
  'ducktape_dataplane_datagrams{service="voice",owner="chat",dir="tx"} 4000',
  'ducktape_dataplane_datagrams{service="voice",owner="chat",dir="rx"} 2000',
  'ducktape_dataplane_drops{service="voice",owner="chat",kind="shed"} 5',
  'ducktape_dataplane_drops{service="voice",owner="chat",kind="rogue_datagrams"} 2',
  'ducktape_dataplane_open{service="gateway",owner="gateway"} 1',
  'ducktape_dataplane_halted{service="gateway",owner="gateway"} 1',
  'ducktape_dataplane_age_seconds{service="gateway",owner="gateway"} 3725',
  'ducktape_dataplane_bytes{service="gateway",owner="gateway",dir="tx",class="stream"} 150000',
  'ducktape_dataplane_bytes{service="gateway",owner="gateway",dir="rx",class="stream"} 98000',
  'ducktape_dataplane_streams{service="gateway",owner="gateway",kind="opened"} 12',
  'ducktape_dataplane_streams{service="gateway",owner="gateway",kind="accepted"} 7',
  "# EOF",
  "",
].join("\n");

const withSyncPeersText = [
  withDataText
    .replace("ducktape_block_height 42", "ducktape_block_height 2000")
    .replace("# EOF\n", ""),
  'ducktape_statesync_serve_age_seconds{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f"} 75',
  'ducktape_statesync_serve_idle_seconds{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f"} 1',
  'ducktape_statesync_serve_bytes{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f"} 5250000',
  'ducktape_statesync_serve_frames{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f"} 40',
  'ducktape_statesync_serve_boundary_height{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f"} 1500',
  'ducktape_statesync_serve_frame_height{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f"} 1540',
  'ducktape_statesync_serve_requests{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f",kind="manifest"} 1',
  'ducktape_statesync_serve_requests{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f",kind="chunk"} 21',
  'ducktape_statesync_serve_requests{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f",kind="frames"} 4',
  'ducktape_statesync_serve_last_request{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f",kind="frames"} 1',
  // a tip poller: no manifest/frames served yet, so no height-shaped series.
  'ducktape_statesync_serve_age_seconds{peer="0011223344556677"} 3000',
  'ducktape_statesync_serve_idle_seconds{peer="0011223344556677"} 9',
  'ducktape_statesync_serve_bytes{peer="0011223344556677"} 4200',
  'ducktape_statesync_serve_frames{peer="0011223344556677"} 0',
  'ducktape_statesync_serve_requests{peer="0011223344556677",kind="tip_coords"} 250',
  'ducktape_statesync_serve_last_request{peer="0011223344556677",kind="tip_coords"} 1',
  "# EOF",
  "",
].join("\n");

// ── Harness: a stub transport whose stream the test drives ──

const streamHarness = () => {
  const subs: TopicHandlers[] = [];
  const subscribe = vi.fn((topics: string[], handlers: TopicHandlers) => {
    if (topics.includes(METRICS_TOPIC)) subs.push(handlers);
    return () => {
      const at = subs.indexOf(handlers);
      if (at >= 0) subs.splice(at, 1);
    };
  });
  const transport = { subscribe } as unknown as NodeTransport;
  const push = (text: string, timeMs = 1_000) =>
    act(() => {
      subs.forEach((handlers) =>
        handlers.onTail?.({
          type: "tail",
          topic: METRICS_TOPIC,
          cursor: String(timeMs),
          item: { timeMs, text },
        }),
      );
    });
  const refuse = () =>
    act(() => {
      subs.forEach((handlers) =>
        handlers.onRefused?.(METRICS_TOPIC, "unknownTopic", "unknown stream topic"),
      );
    });
  return { transport, subscribe, push, refuse };
};

const renderMetrics = (patch: Partial<ConsoleState> = {}) => {
  const harness = streamHarness();
  const state = { ...createInitialState(), connected: true, nodeUrl: "http://n", ...patch };
  const actions = new Proxy(
    {},
    { get: () => vi.fn() },
  ) as ConsoleActions;
  render(
    <ConsoleContext.Provider value={{ state, actions, transport: harness.transport }}>
      <MetricsView />
    </ConsoleContext.Provider>,
  );
  return harness;
};

describe("MetricsView empty states", () => {
  it("says the stream is paused when disconnected (and never subscribes)", () => {
    const { subscribe } = renderMetrics({ connected: false });
    expect(screen.getByText(/Not connected/)).toBeInTheDocument();
    expect(subscribe).not.toHaveBeenCalled();
  });

  it("subscribes the metrics topic and waits for the first sample", () => {
    const { subscribe } = renderMetrics();
    expect(subscribe).toHaveBeenCalledWith([METRICS_TOPIC], expect.anything());
    expect(screen.getByText(/Waiting for the first metrics sample/)).toBeInTheDocument();
  });

  it("is honest when an older daemon refuses the topic", () => {
    const { refuse } = renderMetrics();
    refuse();
    expect(screen.getByText(/doesn't stream metrics/)).toBeInTheDocument();
  });

  it("is honest when the node reports no block metrics (older binary)", () => {
    const { push } = renderMetrics();
    push(runtimeOnlyText);
    expect(screen.getByText(/isn't reporting block metrics/)).toBeInTheDocument();
  });
});

describe("MetricsView charts", () => {
  it("renders the KPIs, latency/throughput panels, and dispatch bars", () => {
    const { push } = renderMetrics();
    push(withDataText);

    // KPI values (unique numbers) land once the first sample arrives.
    expect(screen.getByText("42")).toBeInTheDocument(); // Height
    expect(screen.getByText("100")).toBeInTheDocument(); // Blocks

    // the three panels are present…
    expect(screen.getByText("Apply latency")).toBeInTheDocument();
    expect(screen.getByText("Throughput")).toBeInTheDocument();
    expect(screen.getByText("Dispatches by module")).toBeInTheDocument();
    // latency panel shows its sample count (histogram rendered, not the empty note)
    expect(screen.getByText("3 samples")).toBeInTheDocument();

    // dispatch bars: chat's origins sum to 7, tagging is its own row.
    expect(screen.getByText("chat")).toBeInTheDocument();
    expect(screen.getByText("7")).toBeInTheDocument();
    expect(screen.getByText("tagging")).toBeInTheDocument();

    // none of the empty-state copy is showing.
    expect(screen.queryByText(/isn't reporting block metrics/)).toBeNull();
    expect(screen.queryByText(/Not connected/)).toBeNull();

    // no open planes: the Data planes panel says so honestly.
    expect(screen.getByText("Data planes")).toBeInTheDocument();
    expect(screen.getByText(/No open data planes/)).toBeInTheDocument();

    // no served sync peers: the State sync panel says so honestly.
    expect(screen.getByText("State sync")).toBeInTheDocument();
    expect(screen.getByText(/state-sync lane is idle/)).toBeInTheDocument();
  });

  it("lists every open data plane with its creator and drop accounting", () => {
    const { push } = renderMetrics();
    push(withPlanesText);

    expect(screen.getByText("2 open")).toBeInTheDocument();
    // both planes, attributed to their creating module, with age.
    expect(screen.getByText("voice")).toBeInTheDocument();
    expect(screen.getByText("by chat · open 1m 30s")).toBeInTheDocument();
    expect(screen.getByText("by gateway · open 1h 2m")).toBeInTheDocument();
    // cumulative totals and drop counts per plane.
    expect(screen.getByText("960 kB total")).toBeInTheDocument(); // voice tx+rx
    expect(screen.getByText("248 kB total")).toBeInTheDocument(); // gateway tx+rx
    expect(screen.getByText("7 dropped")).toBeInTheDocument();
    // the gateway plane's pumps stopped: badged.
    expect(screen.getByText("HALTED")).toBeInTheDocument();
    expect(screen.queryByText(/No open data planes/)).toBeNull();
  });

  it("derives throughput from successive samples' counters and instants", () => {
    const { push } = renderMetrics();
    push(withDataText, 1_000);
    // +4 blocks over 2 s of server time → 2.00 blocks/sec in the Rate tile.
    push(withDataText.replace("ducktape_blocks_total 100", "ducktape_blocks_total 104"), 3_000);
    expect(screen.getAllByText("2.00 /s").length).toBeGreaterThan(0);
  });

  it("lists every served sync peer with its phase and block progression", () => {
    const { push } = renderMetrics();
    push(withSyncPeersText);

    expect(screen.getByText("serving 2")).toBeInTheDocument();
    // peers are identified by their leading hex, with the phase under each.
    expect(screen.getByText("9f3ab2c1…")).toBeInTheDocument();
    expect(screen.getByText(/replaying frames · 1m 15s/)).toBeInTheDocument();
    expect(screen.getByText("00112233…")).toBeInTheDocument();
    expect(screen.getByText(/polling tip · 50m 0s/)).toBeInTheDocument();
    // the joiner replayed to 1540 of 2000: 77%, 460 blocks left.
    expect(screen.getByText(/77% · 460 blocks left/)).toBeInTheDocument();
    // the tip poller has no height-shaped responses — an honest placeholder.
    expect(screen.getByText(/no block progression yet/)).toBeInTheDocument();
    // cumulative serve totals per peer.
    expect(screen.getByText(/5.25 MB · 40 frames/)).toBeInTheDocument();
    expect(screen.queryByText(/state-sync lane is idle/)).toBeNull();
  });

  it("renders a joiner that finished and parked as synced, not a regressing sync", () => {
    const { push } = renderMetrics();
    // the joiner's latest ask flips from frames to a routine tip poll.
    push(
      withSyncPeersText.replace(
        'ducktape_statesync_serve_last_request{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f",kind="frames"} 1',
        'ducktape_statesync_serve_last_request{peer="9f3ab2c1d4e5f607a8b9cadbecfd0e1f",kind="tip_coords"} 1',
      ),
    );

    expect(screen.getByText(/parked · 1m 15s/)).toBeInTheDocument();
    expect(screen.getByText("synced")).toBeInTheDocument();
    // the frozen reach is never measured against the advancing goal.
    expect(screen.queryByText(/blocks left/)).toBeNull();
    expect(screen.queryByText(/replaying frames/)).toBeNull();
  });
});
