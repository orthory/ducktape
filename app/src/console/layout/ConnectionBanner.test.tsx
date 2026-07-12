import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ConsoleContext } from "../store/context";
import { createInitialState, type ConnectionDown } from "../store/state";
import type { ConsoleActions } from "../store/actions";
import { ConnectionBanner } from "./ConnectionBanner";

function renderWith(
  down: ConnectionDown | null,
  over: Partial<ConsoleActions> = {},
  managed = true,
) {
  const state = { ...createInitialState(), connectionDown: down, managed };
  return render(
    <ConsoleContext.Provider value={{ state, actions: over as ConsoleActions }}>
      <ConnectionBanner />
    </ConsoleContext.Provider>,
  );
}

describe("ConnectionBanner", () => {
  it("renders nothing while connected", () => {
    const { container } = renderWith(null);
    expect(container.firstChild).toBeNull();
  });

  it("shows the reason and a Restart for a managed node", () => {
    const startNode = vi.fn();
    renderWith({ reason: "could not reach the node (connection refused)" }, { startNode });
    expect(screen.getByText(/could not reach the node/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Restart node" }));
    expect(startNode).toHaveBeenCalledTimes(1);
  });

  it("shows a busy established node without claiming the connection was lost", () => {
    const state = {
      ...createInitialState(),
      connected: true,
      managed: true,
      connectionDown: { reason: "Node is busy — retrying…" },
    };
    render(
      <ConsoleContext.Provider value={{ state, actions: {} as ConsoleActions }}>
        <ConnectionBanner />
      </ConsoleContext.Provider>,
    );
    expect(screen.getByText("Node is busy — retrying…")).toBeTruthy();
    expect(screen.queryByText(/Lost connection/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart node" })).toBeNull();
  });

  it("offers no Restart when a different node grabbed the port (impostor)", () => {
    renderWith({ reason: "a different node is now answering", impostor: true });
    expect(screen.getByText(/a different node is now answering/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Restart node" })).toBeNull();
  });
});
