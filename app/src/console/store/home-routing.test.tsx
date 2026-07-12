// Account-centric Home routing (store slice, Task 7):
//   - smart boot: a desktop with workspaces registered but none active lands at
//     the Home layer (state.atHome), NOT the first-run onboarding gate;
//   - first run (no workspaces at all) still raises onboarding;
//   - goHome() shows Home without tearing down the node connection;
//   - entering a workspace (connectActive) clears atHome.
// Same mocked-invoke + stubbed-node harness as workspace-management.test.tsx.

import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";
import type { Workspace } from "../../domain/workspace-client";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const status = (publicKey?: string) => ({
  version: "0.1.0",
  appHash: "aa".repeat(32),
  height: 0,
  modules: [],
  ...(publicKey ? { publicKey } : {}),
});

const jsonResponse = (code: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status: code,
    headers: { "content-type": "application/json" },
  });

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

/** A stubbed node surface: /v1/status answers with `pubkey`, valset queries
 *  answer by variant, everything else is generic — so refresh() lands a clean
 *  snapshot (connected:true) instead of catching on a valset parse. */
const nodeFetch = (pubkey = "ab12") =>
  vi.fn((url: string, init?: RequestInit) => {
    const u = String(url);
    if (u.endsWith("/v1/status")) return Promise.resolve(jsonResponse(200, status(pubkey)));
    if (u.endsWith("/v1/query")) {
      const body = JSON.parse(String(init?.body ?? "{}")) as { target?: string; query?: unknown };
      if (body.target === "valset" && body.query === "validators") {
        return Promise.resolve(jsonResponse(200, { validators: [[0xab, 0x12]] }));
      }
      if (body.target === "valset" && body.query === "residents") {
        return Promise.resolve(jsonResponse(200, { residents: [] }));
      }
      return Promise.resolve(jsonResponse(200, { channels: [] }));
    }
    return Promise.resolve(jsonResponse(200, { channels: [] }));
  });

const workspace = (over: Partial<Workspace>): Workspace => ({
  id: "team",
  name: "Team",
  chainId: "team#abcd",
  pubkey: "ab12",
  founder: true,
  member: true,
  ports: { listen: 1, http: 9001, rpc: 3 },
  ...over,
});

let actions: ConsoleActions | null = null;

function Probe() {
  const { state, actions: a } = useDucktape();
  actions = a;
  return (
    <div>
      <span data-testid="home">{String(state.atHome)}</span>
      <span data-testid="gate">{String(state.needsOnboarding)}</span>
      <span data-testid="ws">{state.workspace?.name ?? "none"}</span>
      <span data-testid="nodeUrl">{state.nodeUrl ?? "none"}</span>
      <span data-testid="connected">{String(state.connected)}</span>
    </div>
  );
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  invokeMock.mockReset();
  localStorage.clear();
  actions = null;
});

const bootDesktop = (
  list: Workspace[],
  active: Workspace | null,
  handlers: Record<string, (args?: Record<string, unknown>) => unknown> = {},
) => {
  markTauri();
  invokeMock.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    if (cmd in handlers) return Promise.resolve(handlers[cmd](args));
    switch (cmd) {
      case "workspace_list":
        return Promise.resolve(list);
      case "workspace_active":
        return Promise.resolve(active);
      default:
        return Promise.resolve(null);
    }
  });
  render(
    <DucktapeProvider>
      <Probe />
    </DucktapeProvider>,
  );
};

describe("smart boot", () => {
  it("lands at Home (not onboarding) when workspaces exist but none is active", async () => {
    await act(async () => {
      bootDesktop([workspace({})], null);
    });
    await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("true"));
    expect(screen.getByTestId("gate").textContent).toBe("false");
    expect(screen.getByTestId("ws").textContent).toBe("none");
  });

  it("raises onboarding on first run (no workspaces at all)", async () => {
    await act(async () => {
      bootDesktop([], null);
    });
    await waitFor(() => expect(screen.getByTestId("gate").textContent).toBe("true"));
    expect(screen.getByTestId("home").textContent).toBe("false");
  });

  it("clears atHome when a workspace is entered", async () => {
    const team = workspace({});
    await act(async () => {
      bootDesktop([team], null, {
        workspace_select: () => ({ id: "team", httpUrl: "http://127.0.0.1:9001" }),
      });
    });
    await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("true"));
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        String(url).endsWith("/v1/status")
          ? Promise.resolve(jsonResponse(200, status("ab12")))
          : Promise.resolve(jsonResponse(200, { channels: [] })),
      ),
    );

    await act(async () => {
      actions!.selectWorkspace("team");
    });

    await waitFor(() => {
      expect(screen.getByTestId("ws").textContent).toBe("Team");
      expect(screen.getByTestId("home").textContent).toBe("false");
    });
  });
});

describe("goHome", () => {
  it("shows Home without disconnecting the node", async () => {
    const team = workspace({});
    vi.stubGlobal("fetch", nodeFetch());
    await act(async () => {
      bootDesktop([team], team, {
        workspace_select: () => ({ id: "team", httpUrl: "http://127.0.0.1:9001" }),
      });
    });
    // wait until the workspace node is adopted (nodeUrl resolved).
    await waitFor(() => {
      expect(screen.getByTestId("ws").textContent).toBe("Team");
      expect(screen.getByTestId("nodeUrl").textContent).not.toBe("none");
    });
    const url = screen.getByTestId("nodeUrl").textContent;
    const connected = screen.getByTestId("connected").textContent;

    await act(async () => {
      actions!.goHome();
    });

    expect(screen.getByTestId("home").textContent).toBe("true");
    // no disconnect: goHome is a pure view toggle — node url and the live
    // connection flag are left exactly as they were.
    expect(screen.getByTestId("nodeUrl").textContent).toBe(url);
    expect(screen.getByTestId("connected").textContent).toBe(connected);
  });
});
