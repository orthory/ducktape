// The account Home layer: profile + workspace table + custody, with the
// chain-scope banner when nothing is connected. Store comes in through a
// ConsoleContext harness; custody's native fetch through a mocked invoke.

import { render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import type { Workspace } from "../../../domain/workspace-client";
import { HomeView } from "./HomeView";

const DEVICE_KEY = "cd34".repeat(16);

const workspace: Workspace = {
  id: "acme",
  name: "Acme",
  chainId: "acme#abcd1234",
  pubkey: "ab".repeat(32),
  founder: false,
  member: true,
  ports: { listen: 7420, http: 8844, rpc: 9020 },
};

const markNative = () => {
  (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__ = invokeMock;
};

const renderHome = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    author: "Rae",
    workspaces: [workspace],
    ...patch,
  } as ConsoleState;
  const spies: Record<string, Mock<(...args: unknown[]) => unknown>> = {};

  function Harness() {
    const [state, setState] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_t, key: string) => {
          spies[key] ??= vi.fn();
          if (key === "setAuthor")
            return (author: string) => {
              spies[key]?.(author);
              setState((prev) => ({ ...prev, author }));
            };
          return spies[key];
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <HomeView />
      </ConsoleContext.Provider>
    );
  }
  render(<Harness />);
  return { spies };
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__;
  vi.clearAllMocks();
});

describe("HomeView", () => {
  it("shows the profile, the workspace table, and custody; banner when disconnected", async () => {
    markNative();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "user_identity_state")
        return Promise.resolve({ state: "locked", pubkey: DEVICE_KEY, mnemonicConfirmed: true });
      // The Devices card probes Touch ID on mount; keep it inert here.
      if (cmd === "touchid_available" || cmd === "touchid_enrolled")
        return Promise.resolve(false);
      throw new Error(`unexpected invoke ${cmd}`);
    });

    renderHome({ workspace: null, nodeUrl: null, connected: false });

    const content = document.querySelector('[data-home-content="full-width"]') as HTMLElement;
    expect(content).toHaveStyle({ width: "100%" });
    expect(content.style.maxWidth).toBe("");

    // Profile + workspace table are machine-scoped and always render.
    expect(screen.getByDisplayValue("Rae")).toBeInTheDocument();
    expect(screen.getByText("YOUR NETWORKS")).toBeInTheDocument();
    expect(screen.getByText("Acme")).toBeInTheDocument();

    // Disconnected → the honest chain-scope banner.
    expect(screen.getByText(/Account data lives on each network/)).toBeInTheDocument();

    // Custody renders once the identity fetch resolves.
    expect(await screen.findByText("RECOVERY & SECURITY")).toBeInTheDocument();
  });

  it("does not fetch identity on the web build (no native shell)", async () => {
    renderHome();
    await Promise.resolve();
    expect(invokeMock).not.toHaveBeenCalled();
    // The workspace table still renders — it is machine-scoped.
    expect(screen.getByText("YOUR NETWORKS")).toBeInTheDocument();
  });
});
