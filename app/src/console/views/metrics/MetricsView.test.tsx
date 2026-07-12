// The Metrics view polls actions.readMetrics() while mounted and charts the
// node's ducktape_* series — with honest empty states for disconnected and for
// a node that reports only runtime metrics (an older binary).

import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { emptyMetrics, type NodeMetrics } from "../../../domain/metrics";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { MetricsView } from "./MetricsView";

const withData: NodeMetrics = {
  present: true,
  blockHeight: 42,
  blocksTotal: 100,
  latency: {
    sum: 0.03,
    count: 3,
    buckets: [
      { le: 0.001, cumulative: 0 },
      { le: 0.01, cumulative: 2 },
      { le: 0.1, cumulative: 3 },
      { le: Infinity, cumulative: 3 },
    ],
  },
  dispatches: [
    { module: "chat", origin: "external", count: 5 },
    { module: "chat", origin: "module", count: 2 },
    { module: "tagging", origin: "module", count: 3 },
  ],
  planes: [],
  syncPeers: [],
};

const withPlanes: NodeMetrics = {
  ...withData,
  planes: [
    {
      service: "voice",
      owner: "chat",
      ageSeconds: 90,
      halted: false,
      bytesTxDatagram: 640000,
      bytesRxDatagram: 320000,
      bytesTxStream: 0,
      bytesRxStream: 0,
      datagramsTx: 4000,
      datagramsRx: 2000,
      streamsOpened: 0,
      streamsAccepted: 0,
      drops: { shed: 5, rogue_datagrams: 2 },
    },
    {
      service: "gateway",
      owner: "gateway",
      ageSeconds: 3725,
      halted: true,
      bytesTxDatagram: 0,
      bytesRxDatagram: 0,
      bytesTxStream: 150000,
      bytesRxStream: 98000,
      datagramsTx: 0,
      datagramsRx: 0,
      streamsOpened: 12,
      streamsAccepted: 7,
      drops: {},
    },
  ],
};

const withSyncPeers: NodeMetrics = {
  ...withData,
  blockHeight: 2000,
  syncPeers: [
    {
      peer: "9f3ab2c1d4e5f607a8b9cadbecfd0e1f",
      ageSeconds: 75,
      idleSeconds: 1,
      bytesTx: 5_250_000,
      framesServed: 40,
      boundaryHeight: 1500,
      servedHeight: 1540,
      requests: { manifest: 1, chunk: 21, frames: 4 },
    },
    {
      peer: "0011223344556677",
      ageSeconds: 3000,
      idleSeconds: 9,
      bytesTx: 4200,
      framesServed: 0,
      boundaryHeight: null,
      servedHeight: null,
      requests: { tip_coords: 250 },
    },
  ],
};

const renderMetrics = (
  readMetrics: () => Promise<NodeMetrics | null>,
  patch: Partial<ConsoleState> = {},
) => {
  const state = { ...createInitialState(), connected: true, nodeUrl: "http://n", ...patch };
  const actions = new Proxy(
    {},
    {
      get: (_t, key: string) => {
        if (key === "readMetrics") return readMetrics;
        return vi.fn();
      },
    },
  ) as ConsoleActions;
  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <MetricsView />
    </ConsoleContext.Provider>,
  );
};

describe("MetricsView empty states", () => {
  it("says the stream is paused when disconnected (and never polls)", () => {
    const readMetrics = vi.fn().mockResolvedValue(withData);
    renderMetrics(readMetrics, { connected: false });
    expect(screen.getByText(/Not connected/)).toBeInTheDocument();
    expect(readMetrics).not.toHaveBeenCalled();
  });

  it("is honest when the node reports no block metrics (older binary)", async () => {
    renderMetrics(() => Promise.resolve(emptyMetrics()));
    await waitFor(() =>
      expect(screen.getByText(/isn't reporting block metrics/)).toBeInTheDocument(),
    );
  });
});

describe("MetricsView charts", () => {
  it("renders the KPIs, latency/throughput panels, and dispatch bars", async () => {
    renderMetrics(() => Promise.resolve(withData));

    // KPI values (unique numbers) land once the first poll resolves.
    await waitFor(() => expect(screen.getByText("42")).toBeInTheDocument()); // Height
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

  it("lists every open data plane with its creator and drop accounting", async () => {
    renderMetrics(() => Promise.resolve(withPlanes));

    await waitFor(() => expect(screen.getByText("2 open")).toBeInTheDocument());
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

  it("lists every served sync peer with its phase and block progression", async () => {
    renderMetrics(() => Promise.resolve(withSyncPeers));

    await waitFor(() => expect(screen.getByText("serving 2")).toBeInTheDocument());
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
});
