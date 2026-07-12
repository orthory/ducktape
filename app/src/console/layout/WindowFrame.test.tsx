// The title bar's search affordance is a console control: it opens the ⌘K
// palette over a connected workspace. With no workspace chosen (the onboarding
// gate) or mid-join (the waiting room) there is nothing to search, so the bar
// must not render.
//
// The bar's left slot names the window: the active workspace's name when one
// is connected, the "ducktape" brand wherever none exists (web build, remote
// node, the gate) — with the global back/forward pair ahead of it, enabled
// from state.nav (the store's picture of the history-stack position).

import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "../store/DucktapeProvider";
import type { ConsoleActions } from "../store/DucktapeProvider";
import { useDucktape } from "../store/use-ducktape";
import type { Workspace } from "../../domain/workspace-client";
import { WindowFrame } from "./WindowFrame";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  invokeMock.mockReset();
  localStorage.clear();
  // jsdom's session history persists across tests in this file — park the
  // shared top entry on a null state so the next boot starts clean.
  window.history.replaceState(null, "");
});

describe("window frame search affordance", () => {
  it("is hidden while the onboarding gate is up (no workspace chosen)", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) =>
      cmd === "workspace_list" ? Promise.resolve([]) : Promise.resolve(null),
    );

    render(
      <DucktapeProvider>
        <WindowFrame>
          <div />
        </WindowFrame>
      </DucktapeProvider>,
    );

    // boot settles on the raised gate; the search affordance must not appear.
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("workspace_list"));
    expect(screen.queryByLabelText("Search")).toBeNull();
  });

  it("shows once a node is connected (web build resolves one directly)", async () => {
    // no tauri marker: the web build dials its configured node — no onboarding.
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify({ channels: [] }), {
            status: 200,
            headers: { "content-type": "application/json" },
          }),
        ),
      ),
    );

    render(
      <DucktapeProvider>
        <WindowFrame>
          <div />
        </WindowFrame>
      </DucktapeProvider>,
    );

    await waitFor(() => expect(screen.getByLabelText("Search")).toBeTruthy());
    // Non-mac environments (this test env included) must advertise Ctrl, not ⌘.
    expect(screen.getByText("Ctrl K")).toBeTruthy();
  });
});

describe("title bar workspace name", () => {
  const team: Workspace = {
    id: "team",
    name: "Team",
    chainId: "team#abcd",
    pubkey: "ab12",
    founder: true,
    member: true,
    ports: { listen: 1, http: 9001, rpc: 3 },
  };

  const jsonResponse = (body: unknown): Response =>
    new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });

  it("shows the active workspace's name once its node is connected", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "workspace_list":
          return Promise.resolve([team]);
        case "workspace_active":
          return Promise.resolve(team);
        case "workspace_select":
          return Promise.resolve({ id: "team", httpUrl: "http://127.0.0.1:9001" });
        default:
          return Promise.resolve(null);
      }
    });
    // the node answers with the workspace's own identity so the connect sticks.
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        Promise.resolve(
          jsonResponse(
            String(url).endsWith("/v1/status")
              ? {
                  version: "0.1.0",
                  appHash: "aa".repeat(32),
                  height: 0,
                  modules: [],
                  publicKey: "ab12",
                }
              : { channels: [] },
          ),
        ),
      ),
    );

    render(
      <DucktapeProvider>
        <WindowFrame>
          <div />
        </WindowFrame>
      </DucktapeProvider>,
    );

    await waitFor(() => expect(screen.getByText("Team")).toBeTruthy());
    expect(screen.queryByText("ducktape")).toBeNull();
  });

  it("keeps the brand where no workspace exists (web build)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(jsonResponse({ channels: [] }))),
    );

    render(
      <DucktapeProvider>
        <WindowFrame>
          <div />
        </WindowFrame>
      </DucktapeProvider>,
    );

    await waitFor(() => expect(screen.getByLabelText("Search")).toBeTruthy());
    expect(screen.getByText("ducktape")).toBeTruthy();
  });
});

describe("title bar back/forward", () => {
  const jsonResponse = (body: unknown): Response =>
    new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    });

  let actions: ConsoleActions | null = null;

  function Grab() {
    actions = useDucktape().actions;
    return null;
  }

  const button = (label: string) => screen.getByLabelText(label) as HTMLButtonElement;

  /** jsdom performs back()/forward() traversal (and its popstate dispatch) on
   *  a queued task — flush it inside act so the store update lands. */
  const traverse = async (go: () => void) => {
    await act(async () => {
      go();
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  };

  it("walks the console's own entries and disables at the stack edges", async () => {
    // web build with an answering node: the connected shell, no gate.
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(jsonResponse({ channels: [] }))),
    );

    render(
      <DucktapeProvider>
        <WindowFrame>
          <Grab />
        </WindowFrame>
      </DucktapeProvider>,
    );

    // boot holds the single stamped entry — nowhere to go either way.
    await waitFor(() => expect(screen.getByLabelText("Back")).toBeTruthy());
    expect(button("Back").disabled).toBe(true);
    expect(button("Forward").disabled).toBe(true);

    // a screen switch pushes an entry: back opens up.
    await act(async () => {
      actions!.setScreen("members");
    });
    await waitFor(() => expect(button("Back").disabled).toBe(false));
    expect(button("Forward").disabled).toBe(true);

    // clicking Back traverses to the boot entry and flips the pair.
    await traverse(() => fireEvent.click(button("Back")));
    await waitFor(() => expect(button("Forward").disabled).toBe(false));
    expect(button("Back").disabled).toBe(true);

    // and Forward returns to the pushed entry.
    await traverse(() => fireEvent.click(button("Forward")));
    await waitFor(() => expect(button("Back").disabled).toBe(false));
    expect(button("Forward").disabled).toBe(true);
  });
});
