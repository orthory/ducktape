// W1 network rail behavior: the far-left Discord-style switcher. Boots the
// provider over the same mocked-invoke + stubbed-node harness the store suites
// use, renders NetworkRail, and drives its chips: the me chip opens the account
// home, one chip per joined network (join order), the active chip is marked,
// the "+" opens the connect panel, and a client connection gets a badged remote
// seat.

import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "../../console/store/DucktapeProvider";
import { useDucktape } from "../../console/store/use-ducktape";
import type { ConsoleActions } from "../../console/store/DucktapeProvider";
import { NetworkRail } from "../../console/layout/NetworkRail";
import type { Workspace } from "../../domain/workspace-client";

const invokeMock = vi.hoisted(() => vi.fn());

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

const markNative = () => {
  (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__ = invokeMock;
};

const nodeFetch = (pubkey = "ab12") =>
  vi.fn((url: string) =>
    String(url).endsWith("/v1/status")
      ? Promise.resolve(jsonResponse(200, status(pubkey)))
      : Promise.resolve(jsonResponse(200, { channels: [] })),
  );

const workspace = (over: Partial<Workspace>): Workspace => ({
  id: "alpha",
  name: "Alpha",
  chainId: "alpha#0001",
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
      <span data-testid="url">{state.nodeUrl ?? "none"}</span>
    </div>
  );
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__;
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  invokeMock.mockReset();
  localStorage.clear();
  actions = null;
  window.history.replaceState(null, "");
});

const boot = (
  list: Workspace[],
  active: Workspace | null,
  handlers: Record<string, (args?: Record<string, unknown>) => unknown> = {},
) => {
  markNative();
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
      <NetworkRail />
      <Probe />
    </DucktapeProvider>,
  );
};

describe("network rail", () => {
  it("shows the me chip and one chip per joined network in join order", async () => {
    await act(async () => {
      boot([workspace({ id: "alpha", name: "Alpha" }), workspace({ id: "beta", name: "Beta" })], null);
    });
    await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("true"));

    expect(screen.getByRole("button", { name: "Account home" })).toBeTruthy();
    const alpha = screen.getByRole("button", { name: "Alpha" });
    const beta = screen.getByRole("button", { name: "Beta" });
    expect(alpha.textContent).toBe("A");
    expect(beta.textContent).toBe("B");
    // join order: Alpha before Beta in the DOM.
    expect(alpha.compareDocumentPosition(beta) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // nothing entered → no chip is the active (aria-current) one.
    expect(alpha.getAttribute("aria-current")).toBeNull();
  });

  it("the + opens the connect panel", async () => {
    await act(async () => {
      boot([workspace({})], null);
    });
    await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("true"));

    await act(async () => {
      screen.getByRole("button", { name: "Add a network" }).click();
    });
    expect(screen.getByTestId("gate").textContent).toBe("true");
  });

  it("marks the active network and the me chip opens the account home", async () => {
    const alpha = workspace({ id: "alpha", name: "Alpha" });
    vi.stubGlobal("fetch", nodeFetch());
    await act(async () => {
      boot([alpha], alpha, {
        workspace_select: () => ({ id: "alpha", httpUrl: "http://127.0.0.1:9001" }),
      });
    });
    // connected: not at home, the active chip is current.
    await waitFor(() => expect(screen.getByTestId("ws").textContent).toBe("Alpha"));
    expect(screen.getByTestId("home").textContent).toBe("false");
    expect(screen.getByRole("button", { name: "Alpha" }).getAttribute("aria-current")).toBe("true");

    await act(async () => {
      screen.getByRole("button", { name: "Account home" }).click();
    });
    expect(screen.getByTestId("home").textContent).toBe("true");
    // at home the me chip is the active surface — the connected network keeps
    // its seat but drops its active ring (exactly one chip reads active).
    expect(screen.getByRole("button", { name: "Alpha" }).getAttribute("aria-current")).toBeNull();
  });

  it("gives a client connection a badged remote seat", async () => {
    vi.stubGlobal("fetch", nodeFetch());
    await act(async () => {
      boot([], null);
    });
    await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("true"));

    await act(async () => {
      actions!.connectRemote("http://10.0.0.5:8844");
    });
    await waitFor(() => expect(screen.getByTestId("url").textContent).toBe("http://10.0.0.5:8844"));
    // the remote seat is present, titled as remote, and active (the live one).
    const remote = screen.getByRole("button", { name: /remote/i });
    expect(remote.getAttribute("aria-current")).toBe("true");
  });
});
