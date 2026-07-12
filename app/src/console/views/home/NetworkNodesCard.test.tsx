import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { NetworkNodesCard } from "./NetworkNodesCard";

const accountUnbindNode = vi.fn().mockResolvedValue(undefined);
const state = {
  workspace: { id: "w1", pubkey: "aa11", name: "orthory", chainId: "duck-1" },
  members: ["aa11"],
  residents: [] as string[],
  workspaces: [],
  nodeUsers: { aa11: { accountId: "acct-1" }, bb22: { accountId: "acct-1" } },
};

vi.mock("../../store/use-ducktape", () => ({
  useDucktape: () => ({ state, actions: { accountUnbindNode } }),
}));

describe("NetworkNodesCard", () => {
  it("renders nothing without an account", () => {
    const { container } = render(<NetworkNodesCard accountId={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("lists the account's nodes and Unbind evicts via accountUnbindNode", () => {
    render(<NetworkNodesCard accountId="acct-1" />);
    // two nodes bound to acct-1
    const unbinds = screen.getAllByRole("button", { name: /Unbind node/ });
    expect(unbinds).toHaveLength(2);
    fireEvent.click(unbinds[0]);
    // confirm dialog
    fireEvent.click(screen.getByRole("button", { name: "Unbind node" }));
    expect(accountUnbindNode).toHaveBeenCalledOnce();
  });
});
