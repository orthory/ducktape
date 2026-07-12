import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { BlockDisposition, BlockRecord } from "../../../domain/transport";
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

const PEER_B = "11".repeat(32);
const RESIDENT_C = "22".repeat(32);

const block = (
  height: number,
  proposer: string,
  disposition: BlockDisposition = "applied",
): BlockRecord => ({
  height,
  hash: `hash${height}`,
  commitHash: `commit${height}`,
  ops: [{ proposer, disposition, target: "chat", operations: [], payload: "", opHash: "" }],
});

const renderStatus = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    managed: true,
    workspace,
    status,
    ...patch,
  };
  const spies: Record<string, (...args: unknown[]) => unknown> = {};
  const actions = new Proxy(
    {},
    {
      get: (_target, key: string) => {
        spies[key] ??= vi.fn() as (...args: unknown[]) => unknown;
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

    renderStatus({ members: [workspace.pubkey, PEER_B], residents: [RESIDENT_C] });

    expect(screen.getByText("Synced")).toBeInTheDocument();
    expect(screen.getByText(/member · validator/i)).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    // The network cards now report real, fetched counts rather than stubs.
    expect(screen.getByText("VALIDATORS")).toBeInTheDocument();
    expect(screen.getByText("RESIDENTS")).toBeInTheDocument();
    expect(screen.getByText("CADENCE")).toBeInTheDocument();
    expect(screen.getByText("COMMIT HEALTH")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /app hash/i }));

    expect(writeText).toHaveBeenCalledWith(status.appHash);
    expect(screen.getByText("COPIED")).toBeInTheDocument();
    expect(screen.getByText("chat")).toBeInTheDocument();
    expect(screen.getByText("tasks")).toBeInTheDocument();
  });

  it("shows the node ops facts that moved here from Settings", () => {
    renderStatus({ members: [workspace.pubkey, PEER_B, RESIDENT_C] });

    expect(screen.getByText("Data dir")).toBeInTheDocument();
    expect(
      screen.getByText("~/.ducktape/workspaces/acme-research"),
    ).toBeInTheDocument();
    expect(screen.getByText("Ports")).toBeInTheDocument();
    expect(screen.getByText("p2p 7420 · http 8844 · rpc 9020")).toBeInTheDocument();
    expect(screen.getByText("Quorum threshold")).toBeInTheDocument();
    // floor(3 * 2/3) + 1 = 3 of the 3 validators.
    expect(screen.getByText("3 of 3 validators")).toBeInTheDocument();
  });

  it("lists connections with derived liveness on the Connections tab", () => {
    renderStatus({
      members: [workspace.pubkey, PEER_B],
      residents: [RESIDENT_C],
      authorNames: { [PEER_B]: "beacon" },
      // PEER_B led the two recent blocks; self led none.
      blocks: [block(41, PEER_B), block(42, PEER_B)],
      lastBlock: 42,
    });

    fireEvent.click(screen.getByRole("button", { name: "Connections" }));

    // The valset roster is fetched and shown as this node's connections.
    expect(screen.getByText("CONNECTIONS")).toBeInTheDocument();
    expect(screen.getByText("beacon")).toBeInTheDocument();
    expect(screen.getByText("this node")).toBeInTheDocument();

    // A validator that verifiably proposed recent blocks reads as leading;
    // the local validator that led nothing reads as quiet; residents as statesync.
    expect(screen.getByText("leading")).toBeInTheDocument();
    expect(screen.getByText("quiet")).toBeInTheDocument();
    expect(screen.getByText("statesync")).toBeInTheDocument();
    expect(screen.getByText(/led #42/)).toBeInTheDocument();

    // Resident tier is disjoint from the quorum and labelled as such.
    expect(screen.getAllByText("validator").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("resident")).toBeInTheDocument();
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
