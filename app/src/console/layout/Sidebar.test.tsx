// The module-nav rail (Sidebar): the view-mode toggle and per-module entries.
// The account "me" chip lives on the far-left network rail now (epic W1), not
// here — see test/sim/rail.test.tsx. Store comes in through a ConsoleContext
// harness.

import { render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi, type Mock } from "vitest";

import type { ConsoleActions } from "../store/actions";
import { ConsoleContext } from "../store/context";
import { createInitialState, type ConsoleState } from "../store/state";
import { Sidebar } from "./Sidebar";

const renderSidebar = (patch: Partial<ConsoleState> = {}) => {
  const initialState = { ...createInitialState(), author: "Rae", ...patch } as ConsoleState;
  const spies: Record<string, Mock<(...args: unknown[]) => unknown>> = {};
  function Harness() {
    const [state] = useState(initialState);
    const actions = new Proxy(
      {},
      { get: (_t, key: string) => (spies[key] ??= vi.fn()) },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <Sidebar />
      </ConsoleContext.Provider>
    );
  }
  render(<Harness />);
  return { spies };
};

describe("Sidebar", () => {
  const railBg = () =>
    (screen.getByRole("button", { name: "Chat" }) as HTMLButtonElement).style
      .background;

  it("highlights the routed screen's rail entry under the shell", () => {
    renderSidebar({ screen: "chat", atHome: false });
    expect(railBg()).not.toBe("transparent");
  });

  it("at Home only the avatar highlights — never the covered rail screen", () => {
    // the Home layer covers the routed chat screen: its rail entry must not
    // claim to be the visible surface.
    renderSidebar({ screen: "chat", atHome: true });
    expect(railBg()).toBe("transparent");
  });

  it("shows a remote client the account rail only — no view-mode toggle", () => {
    renderSidebar({ workspace: null, nodeUrl: "https://node.example" });

    expect(screen.queryByRole("tab", { name: "USER" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "NODE" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Forge" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Explorer" })).toBeInTheDocument();
    // A3-pending surfaces stay off the client rail for now.
    expect(screen.queryByRole("button", { name: "Members" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Governance" })).not.toBeInTheDocument();
  });

  it("a persisted operator mode falls back to the account rail without node control", () => {
    renderSidebar({
      workspace: null,
      nodeUrl: "https://node.example",
      viewMode: "operator",
      screen: "status",
    });

    // The NODE surface is absent, not disabled (ADR A5/A6).
    expect(screen.queryByRole("button", { name: "Node" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Metrics" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Chat" })).toBeInTheDocument();
  });

  it("reveals the USER/NODE toggle only with node control", () => {
    renderSidebar({
      workspace: { id: "w" } as unknown as ConsoleState["workspace"],
      managed: true,
    });

    expect(screen.getByRole("tab", { name: "USER" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "NODE" })).toHaveAttribute("title", "Node operator");
  });
});
