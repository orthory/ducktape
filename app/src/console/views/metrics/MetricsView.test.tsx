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
  });
});
