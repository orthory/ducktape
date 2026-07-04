import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi, type Mock } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Workspace } from "../../../domain/workspace-client";
import { SettingsView } from "./SettingsView";

const workspace: Workspace = {
  id: "acme-research",
  name: "Acme Research",
  chainId: "acme#abcd1234",
  pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
  founder: false,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const renderSettings = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    author: "Rae",
    workspace,
    managed: true,
    connected: true,
    ...patch,
  };
  const spies: Record<string, Mock<(...args: unknown[]) => void>> = {};
  const noop: Mock<(...args: unknown[]) => void> = vi.fn();

  function Harness() {
    const [state, setState] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn();
          if (key === "setAuthor") {
            return (author: string) => {
              spies[key]?.(author);
              setState((prev) => ({ ...prev, author }));
            };
          }
          if (key === "setAccent") {
            return (accent: string) => {
              spies[key]?.(accent);
              setState((prev) => ({ ...prev, accent }));
            };
          }
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <SettingsView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

describe("SettingsView", () => {
  it("renders the workspace settings and preserves the existing actions", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const { spies } = renderSettings();

    expect(screen.getByText("NETWORK")).toBeInTheDocument();
    expect(screen.getByText("Acme Research")).toBeInTheDocument();
    expect(screen.getByText("~/.ducktape/workspaces/acme-research")).toBeInTheDocument();
    expect(screen.getByText(/p2p 7420/i)).toBeInTheDocument();
    expect(screen.getByText(/member validator/i)).toBeInTheDocument();

    expect(screen.getByText("YOUR IDENTITY")).toBeInTheDocument();
    expect(screen.getByText(/abcdef012345/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /copy key/i }));
    expect(writeText).toHaveBeenCalledWith(workspace.pubkey);

    const name = screen.getByDisplayValue("Rae");
    fireEvent.change(name, { target: { value: "Ari" } });
    expect(spies.setAuthor).toHaveBeenCalledWith("Ari");
    fireEvent.blur(name);
    expect(spies.setDisplayName).toHaveBeenCalledWith("Ari");

    fireEvent.click(screen.getByRole("button", { name: /set accent #3d63b8/i }));
    expect(spies.setAccent).toHaveBeenCalledWith("#3d63b8");

    fireEvent.click(screen.getByRole("button", { name: /workspaces/i }));
    expect(spies.newWorkspace).toHaveBeenCalled();
  });

  it("leaves the network on a confirmed danger-zone click", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { spies } = renderSettings();

    // The copy is honest: an on-chain self-removal, pending remaining members.
    expect(screen.getByText(/on-chain self-removal/i)).toBeInTheDocument();
    expect(
      screen.queryByText(/full workspace deletion is not wired/i),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /leave network/i }));
    expect(confirm).toHaveBeenCalledOnce();
    expect(spies.leaveWorkspace).toHaveBeenCalled();
    // The old dishonest wiring is gone — stopNode is no longer the leave path.
    expect(spies.stopNode?.mock.calls ?? []).toHaveLength(0);

    confirm.mockRestore();
  });

  it("aborts the leave when the confirm is dismissed", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { spies } = renderSettings();

    fireEvent.click(screen.getByRole("button", { name: /leave network/i }));
    expect(confirm).toHaveBeenCalledOnce();
    // Dismissed confirm -> leaveWorkspace is never referenced, so the proxy
    // never even created a spy for it.
    expect(spies.leaveWorkspace?.mock.calls ?? []).toHaveLength(0);

    confirm.mockRestore();
  });
});
