import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ConsoleContext } from "../../store/context";
import { createInitialState, type BootError } from "../../store/state";
import type { ConsoleActions } from "../../store/actions";
import { NodeFailed } from "./NodeFailed";

const boot: BootError = {
  workspaceId: "team",
  reason: "the node for “Team” exited on start: address already in use",
  logPath: "/home/x/.ducktape/workspaces/team/daemon.log",
  logTail: "FATAL bind 127.0.0.1:8844: address already in use",
};

function renderWith(over: Partial<ConsoleActions>) {
  const state = {
    ...createInitialState(),
    bootError: boot,
    workspace: {
      id: "team",
      name: "Team",
      chainId: "team#abcd",
      pubkey: "ab12",
      founder: true,
      member: true,
      ports: { listen: 1, http: 2, rpc: 3 },
    },
  };
  return render(
    <ConsoleContext.Provider value={{ state, actions: over as ConsoleActions }}>
      <NodeFailed />
    </ConsoleContext.Provider>,
  );
}

describe("NodeFailed", () => {
  it("shows the real reason and the log path", () => {
    renderWith({});
    expect(screen.getByText(/address already in use/)).toBeTruthy();
    expect(screen.getByText(boot.logPath!)).toBeTruthy();
  });

  it("Retry re-connects the same workspace (idempotent)", () => {
    const retryConnect = vi.fn();
    renderWith({ retryConnect });
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(retryConnect).toHaveBeenCalledTimes(1);
  });

  it("Open daemon.log reveals the tail on demand", () => {
    renderWith({});
    // hidden until asked
    expect(screen.queryByText(/FATAL bind 127\.0\.0\.1:8844/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /daemon\.log/i }));
    expect(screen.getByText(/FATAL bind 127\.0\.0\.1:8844/)).toBeTruthy();
  });
});
