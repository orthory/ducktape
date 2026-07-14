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

  it("shows the friendly line (raw reason in the tooltip only) and a Restart", () => {
    const startNode = vi.fn();
    renderWith({ reason: "stream socket closed" }, { startNode });
    // the visible copy is the friendly line alone — the raw transport reason
    // is tooltip/log material, never banner text (epic QA BUG-5).
    const line = screen.getByText("Lost connection to the node — reconnecting…");
    expect(line.getAttribute("title")).toBe("stream socket closed");
    expect(screen.queryByText(/stream socket closed/)).toBeNull();
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
