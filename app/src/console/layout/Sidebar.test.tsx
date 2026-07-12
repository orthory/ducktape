// The icon rail's account avatar opens the account Home (a full-window layer,
// not a rail screen), so it routes through actions.goHome rather than
// setScreen("account"). Store comes in through a ConsoleContext harness.

import { fireEvent, render, screen } from "@testing-library/react";
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
  it("avatar opens Home via goHome, not a rail screen", () => {
    const { spies } = renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: /account/i }));
    expect(spies.goHome).toHaveBeenCalled();
    // The avatar no longer routes through the rail's setScreen("account").
    expect(spies.setScreen?.mock.calls ?? []).not.toContainEqual(["account"]);
  });
});
