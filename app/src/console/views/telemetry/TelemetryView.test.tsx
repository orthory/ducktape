// The telemetry timeline's empty state is honest about WHY it is empty:
// disconnected, connected-but-no-history, or history-exists-but-no-live-stream
// (the last cross-links to the Explorer). A populated ring renders frame rows.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { BlockRecord, TelemetryFrame } from "../../../domain/transport";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { TelemetryView } from "./TelemetryView";

const blockRecord = (height: number): BlockRecord => ({
  height,
  hash: "aa".repeat(32),
  commitHash: "bb".repeat(32),
  proposer: "cc".repeat(32),
  disposition: "applied",
  target: "chat",
  operations: [{ module: "chat", origin: "external", emittedMsgs: 0, emittedEvents: 0 }],
  payload: '{"post":{}}',
  opHash: "dd".repeat(32),
});

const frame = (height: number): TelemetryFrame => ({
  height,
  consensusTime: height,
  latencyUs: 512,
  dispatches: [{ module: "chat", origin: "external", emittedMsgs: 1, emittedEvents: 0 }],
  events: [],
});

const renderTelemetry = (patch: Partial<ConsoleState> = {}) => {
  const state = { ...createInitialState(), ...patch };
  const spies: Record<string, ReturnType<typeof vi.fn>> = {};
  const actions = new Proxy(
    {},
    {
      get: (_target, key: string) => {
        spies[key] ??= vi.fn();
        return spies[key];
      },
    },
  ) as ConsoleActions;

  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <TelemetryView />
    </ConsoleContext.Provider>,
  );

  return { spies };
};

describe("TelemetryView empty state", () => {
  it("says the stream is paused when disconnected", () => {
    renderTelemetry({ connected: false, telemetry: [], blocks: [] });
    expect(screen.getByText(/Not connected\. Telemetry is a live stream/)).toBeInTheDocument();
  });

  it("waits for the first block when connected with no committed history", () => {
    renderTelemetry({ connected: true, telemetry: [], blocks: [] });
    expect(screen.getByText(/Waiting for the first block/)).toBeInTheDocument();
  });

  it("points at the Explorer when history exists but no telemetry has streamed", () => {
    const { spies } = renderTelemetry({
      connected: true,
      telemetry: [],
      blocks: [blockRecord(7)],
    });
    expect(screen.getByText(/No live telemetry on this connection yet/)).toBeInTheDocument();
    // the cross-link is actionable, not just prose.
    fireEvent.click(screen.getByRole("button", { name: "Explorer" }));
    expect(spies.setScreen).toHaveBeenCalledWith("explorer");
  });
});

describe("TelemetryView populated", () => {
  it("renders a frame row per telemetry frame and drops the empty state", () => {
    renderTelemetry({ connected: true, telemetry: [frame(7)], blocks: [blockRecord(7)] });
    expect(screen.getByText("#7")).toBeInTheDocument();
    expect(screen.queryByText(/No live telemetry on this connection yet/)).toBeNull();
    expect(screen.queryByText(/Waiting for the first block/)).toBeNull();
  });
});
