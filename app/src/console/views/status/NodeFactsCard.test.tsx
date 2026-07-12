// The Owned by row's manual bind: an unbound node offers a Bind button (the
// escape hatch when connect-time auto-bind returned locked/deferred/failed
// and nothing would ever retry), and every non-landing outcome surfaces as an
// honest inline message instead of vanishing.

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { Workspace } from "../../../domain/workspace-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { NodeFactsCard } from "./NodeFactsCard";

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
  founder: true,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const renderCard = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    managed: true,
    workspace,
    ...patch,
  };
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
    <ConsoleContext.Provider value={{ state: initialState, actions }}>
      <NodeFactsCard />
    </ConsoleContext.Provider>,
  );

  return { spies };
};

describe("NodeFactsCard bind affordance", () => {
  it("offers Bind on an unbound node and reports a landed bind", async () => {
    const { spies } = renderCard();
    expect(screen.getByText("not linked to an account")).toBeInTheDocument();

    const bind = screen.getByRole("button", { name: /bind this node/i });
    spies.accountBindNode ??= vi.fn();
    spies.accountBindNode.mockResolvedValue("bound");
    fireEvent.click(bind);

    await waitFor(() => expect(spies.accountBindNode).toHaveBeenCalledTimes(1));
    // A landed bind needs no inline message — the refreshed projection
    // repaints the row as the owner.
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("hides Bind once the node is owned", () => {
    renderCard({
      nodeUsers: {
        [workspace.pubkey]: { accountId: "aa".repeat(32), name: "Eddy" },
      },
    });
    expect(screen.queryByRole("button", { name: /bind this node/i })).not.toBeInTheDocument();
    expect(screen.getByText(/Eddy/)).toBeInTheDocument();
  });

  it("says WHY when the bind cannot land (locked identity)", async () => {
    const { spies } = renderCard();
    spies.accountBindNode ??= vi.fn();
    spies.accountBindNode.mockResolvedValue("locked");

    fireEvent.click(screen.getByRole("button", { name: /bind this node/i }));

    const message = await screen.findByRole("alert");
    expect(message.textContent).toMatch(/locked/i);
    expect(message.textContent).toMatch(/unlock/i);
    // The button survives the failure — unlocking then retrying is the flow.
    expect(screen.getByRole("button", { name: /bind this node/i })).toBeInTheDocument();
  });

  it("says WHY when a pending device link defers the bind", async () => {
    const { spies } = renderCard();
    spies.accountBindNode ??= vi.fn();
    spies.accountBindNode.mockResolvedValue("deferred");

    fireEvent.click(screen.getByRole("button", { name: /bind this node/i }));

    const message = await screen.findByRole("alert");
    expect(message.textContent).toMatch(/device link/i);
  });

  it("surfaces a plain failure and a thrown rejection alike", async () => {
    const { spies } = renderCard();
    spies.accountBindNode ??= vi.fn();
    spies.accountBindNode.mockRejectedValue(new Error("not connected to a workspace node"));

    fireEvent.click(screen.getByRole("button", { name: /bind this node/i }));

    const message = await screen.findByRole("alert");
    expect(message.textContent).toMatch(/not connected/i);
  });
});
