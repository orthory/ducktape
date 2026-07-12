import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ConsoleContext } from "../../store/context";
import { createInitialState, type BootError } from "../../store/state";
import type { ConsoleActions } from "../../store/actions";
import { NodeFailed } from "./NodeFailed";

const boot: BootError = {
  kind: "startup_failure",
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

function renderIncompatible(over: Partial<ConsoleActions>) {
  const state = {
    ...createInitialState(),
    bootError: { ...boot, kind: "incompatible_workspace" as const },
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

  it("classifies incompatible state as archive-and-create-fresh, never retry", () => {
    const retryConnect = vi.fn();
    const newWorkspace = vi.fn();
    renderIncompatible({ retryConnect, newWorkspace });

    expect(screen.getByText(/Workspace update required/)).toBeTruthy();
    expect(screen.getByText(/data has not been changed/)).toBeTruthy();
    expect(screen.getByText(/node identity, and Ducktape account key/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Create fresh workspace" }));
    expect(newWorkspace).toHaveBeenCalledTimes(1);
    expect(retryConnect).not.toHaveBeenCalled();
  });
});
