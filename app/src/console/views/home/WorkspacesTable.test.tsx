// The Home workspace table: a row per known workspace, the active one marked,
// and Enter routing through actions.selectWorkspace. Store comes in through a
// ConsoleContext harness with Proxy-spied actions (the store hook is the seam).

import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi, type Mock } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Workspace } from "../../../domain/workspace-client";
import { WorkspacesTable } from "./WorkspacesTable";

const ws = (id: string, name: string): Workspace => ({
  id,
  name,
  chainId: `${name.toLowerCase()}#abcd1234`,
  pubkey: id.repeat(64).slice(0, 64),
  founder: false,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
});

const ACME = ws("a", "Acme");
const BETA = ws("b", "Beta");

const renderTable = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    workspaces: [ACME, BETA],
    workspace: ACME,
    members: [ACME.pubkey],
    ...patch,
  } as ConsoleState;
  const spies: Record<string, Mock<(...args: unknown[]) => void>> = {};

  function Harness() {
    const [state] = useState(initialState);
    const actions = new Proxy(
      {},
      { get: (_t, key: string) => (spies[key] ??= vi.fn()) },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <WorkspacesTable />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);
  return { spies };
};

describe("WorkspacesTable", () => {
  it("renders a row per workspace, marks the active one, and Enter selects it", () => {
    const { spies } = renderTable();

    expect(screen.getByText("Acme")).toBeInTheDocument();
    expect(screen.getByText("Beta")).toBeInTheDocument();

    // The active workspace shows its standing; inactive rows show "—".
    expect(screen.getByText("Validator")).toBeInTheDocument();

    // Enter on the inactive workspace routes through selectWorkspace.
    fireEvent.click(screen.getByRole("button", { name: /enter beta/i }));
    expect(spies.selectWorkspace).toHaveBeenCalledWith("b");
  });

  it("does not offer Enter for the already-active workspace", () => {
    renderTable();
    expect(screen.queryByRole("button", { name: /enter acme/i })).not.toBeInTheDocument();
  });

  it("selects the workspace when its row is clicked", () => {
    const { spies } = renderTable();

    fireEvent.click(screen.getByText("Beta"));
    expect(spies.selectWorkspace).toHaveBeenCalledExactlyOnceWith("b");
  });

  it("clicking the active row, or the Enter button, does not double-select", () => {
    const { spies } = renderTable();

    // The active row is inert, and the Enter button's click must not ALSO fire
    // the row handler: connectActive drops the transport, so a second call
    // would reconnect mid-connect. Two clicks, exactly one selection.
    fireEvent.click(screen.getByText("Acme"));
    fireEvent.click(screen.getByRole("button", { name: /enter beta/i }));
    expect(spies.selectWorkspace).toHaveBeenCalledExactlyOnceWith("b");
  });
});
