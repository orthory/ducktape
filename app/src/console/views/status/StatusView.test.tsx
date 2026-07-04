import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Workspace } from "../../../domain/workspace-client";
import { StatusView } from "./StatusView";

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
  founder: false,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const status = {
  version: "0.1.0",
  height: 42,
  appHash: "aa".repeat(32),
  modules: [
    { id: "chat", root: "bb".repeat(32) },
    { id: "tasks", root: "cc".repeat(32) },
  ],
};

const renderStatus = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    managed: true,
    workspace,
    status,
    ...patch,
  };
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const actions = new Proxy(
    {},
    {
      get: (_target, key: string) => {
        spies[key] ??= vi.fn() as (...args: unknown[]) => void;
        return spies[key];
      },
    },
  ) as ConsoleActions;

  render(
    <ConsoleContext.Provider value={{ state: initialState, actions }}>
      <StatusView />
    </ConsoleContext.Provider>,
  );

  return { spies };
};

describe("StatusView", () => {
  it("renders real node state and copies committed roots", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    renderStatus();

    expect(screen.getByText("Synced")).toBeInTheDocument();
    expect(screen.getByText(/member · validator/i)).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getAllByText(/not exposed by \/v1\/status/i)).toHaveLength(3);

    fireEvent.click(screen.getByRole("button", { name: /app hash/i }));

    expect(writeText).toHaveBeenCalledWith(status.appHash);
    expect(screen.getByText("COPIED")).toBeInTheDocument();
    expect(screen.getByText("chat")).toBeInTheDocument();
    expect(screen.getByText("tasks")).toBeInTheDocument();
  });

  it("shows a real validator-vs-guest capability matrix", () => {
    renderStatus();

    fireEvent.click(screen.getByRole("button", { name: "Permissions" }));

    const matrix = screen.getByRole("table", { name: /node capability matrix/i });
    expect(within(matrix).getByText("Validator")).toBeInTheDocument();
    expect(within(matrix).getByText("Guest client")).toBeInTheDocument();

    expect(within(matrix).getByText("Read committed node status")).toBeInTheDocument();
    expect(within(matrix).getByText("Inspect app hash and module roots")).toBeInTheDocument();
    expect(within(matrix).getByText("Submit module messages")).toBeInTheDocument();
    expect(within(matrix).getByText("Start/stop managed daemon")).toBeInTheDocument();
    expect(within(matrix).getByText("Admit waiting workspaces")).toBeInTheDocument();
  });
});
