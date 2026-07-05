import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { BlockRecord } from "../../../domain/transport";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { ExplorerView } from "./ExplorerView";

const proposerKey = "cc".repeat(32);

const block = (height: number, patch: Partial<BlockRecord> = {}): BlockRecord => ({
  height,
  hash: "aa".repeat(32),
  commitHash: "bb".repeat(32),
  proposer: proposerKey,
  disposition: "applied",
  target: "chat",
  operations: [
    { module: "chat", origin: "external", emittedMsgs: 0, emittedEvents: 0 },
  ],
  payload: '{"Post":{}}',
  opHash: "dd".repeat(32),
  ...patch,
});

const renderExplorer = (patch: Partial<ConsoleState> = {}) => {
  const state = { ...createInitialState(), connected: true, ...patch };
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
      <ExplorerView />
    </ConsoleContext.Provider>,
  );

  return { spies };
};

describe("ExplorerView", () => {
  it("opens a block and shows the op's content address alongside the digests", () => {
    renderExplorer({ blocks: [block(7)] });
    fireEvent.click(screen.getByText("#7"));
    expect(screen.getByText("OP HASH")).toBeInTheDocument();
    expect(screen.getByText("dd".repeat(32))).toBeInTheDocument();
  });

  it("renders a ring written before op hashes existed as an empty digest line", () => {
    renderExplorer({ blocks: [block(7, { opHash: undefined })] });
    fireEvent.click(screen.getByText("#7"));
    const label = screen.getByText("OP HASH");
    expect(label.nextElementSibling).toHaveTextContent("—");
  });

  it("consumes a cross-link hand-off: opens the focused block and clears it", () => {
    const { spies } = renderExplorer({
      blocks: [block(7), block(9)],
      explorerFocus: 9,
    });
    // landed straight in the detail — the list never flashed.
    expect(screen.getByText("← Blocks")).toBeInTheDocument();
    expect(screen.getByText("#9")).toBeInTheDocument();
    expect(spies.clearExplorerFocus).toHaveBeenCalled();
  });

  it("keeps a focus pending while the ring has not arrived yet", () => {
    const { spies } = renderExplorer({ blocks: [], explorerFocus: 9 });
    expect(spies.clearExplorerFocus).toBeUndefined();
  });

  it("falls back to the list when the ring evicted the focused height", () => {
    const { spies } = renderExplorer({ blocks: [block(7)], explorerFocus: 999 });
    expect(screen.queryByText("← Blocks")).not.toBeInTheDocument();
    expect(spies.clearExplorerFocus).toHaveBeenCalled();
  });

  it("resolves a proposer to its profile display name and falls back to hex", () => {
    const anonKey = "ee".repeat(32);
    renderExplorer({
      blocks: [block(7), block(8, { proposer: anonKey })],
      authorNames: { [proposerKey]: "Founder Rae" },
    });
    expect(screen.getByText("Founder Rae")).toBeInTheDocument();
    // Unregistered proposer stays the truncated key.
    expect(screen.getByText(`${anonKey.slice(0, 10)}…`)).toBeInTheDocument();
  });

  it("shows the resolved name alongside the full key in the block detail", () => {
    renderExplorer({
      blocks: [block(7)],
      authorNames: { [proposerKey]: "Founder Rae" },
    });
    fireEvent.click(screen.getByText("Founder Rae"));
    expect(screen.getByText(`Founder Rae · ${proposerKey}`)).toBeInTheDocument();
  });
});
