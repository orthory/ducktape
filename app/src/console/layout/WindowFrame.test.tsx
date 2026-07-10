// The title bar's search affordance is a console control: it opens the ⌘K
// palette over a connected workspace. With no workspace chosen (the onboarding
// gate) or mid-join (the waiting room) there is nothing to search, so the bar
// must not render.
//
// The bar's left slot names the window: the active workspace's name when one
// is connected, the "ducktape" brand wherever none exists (web build, remote
// node, the gate).

import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "../store/DucktapeProvider";
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
