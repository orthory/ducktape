// Workspace management contract, driven over the mocked native `invoke` + a
// stubbed node surface (same harness as onboarding.test.tsx):
//   - a not-admitted workspace refuses entry from the picker with an error and
//     stays put — it never repoints the registry or falls through to another
//     workspace's console;
//   - the fresh-join flow seeds the waiting-room phase synchronously, so the
//     console shell (with another workspace's residue) can never flash;
//   - a node answering a workspace's port with the WRONG identity is not
//     adopted — the connect backs out honestly;
//   - any workspace can be deleted from the picker by id, with the force
//     escalation scoped to that workspace.

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";
import type { Workspace } from "../../domain/workspace-client";
import { ConnectPanel } from "../views/onboarding/ConnectPanel";

const invokeMock = vi.hoisted(() => vi.fn());

// The native event plane, mocked so a test can play the Rust side and fire
// shell events (identity-unlocked) into whatever listeners the provider hung.
const nativeEvents = vi.hoisted(() => {
  const handlers = new Map<string, Set<(event: { payload: unknown }) => void>>();
  return {
    handlers,
    /** Fire a native event into every registered listener (the test's Rust). */
    emitTo(name: string, payload: unknown) {
      handlers.get(name)?.forEach((handler) => handler({ payload }));
    },
    listen: vi.fn((name: string, handler: (event: { payload: unknown }) => void) => {
      if (!handlers.has(name)) handlers.set(name, new Set());
      handlers.get(name)!.add(handler);
      return Promise.resolve(() => handlers.get(name)?.delete(handler));
    }),
    emit: vi.fn(() => Promise.resolve()),
  };
});
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
  (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_EVENTS__ = nativeEvents;
};

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
      <span data-testid="gate">{String(state.needsOnboarding)}</span>
      <span data-testid="home">{String(state.atHome)}</span>
      <span data-testid="ws">{state.workspace?.name ?? "none"}</span>
      <span data-testid="phase">{state.onboardingPhase?.phase ?? "none"}</span>
      <span data-testid="error">{state.error ?? "none"}</span>
      <span data-testid="list">{state.workspaces.map((w) => w.id).join(",")}</span>
      <span data-testid="needs-force">{state.deleteNeedsForce ?? "none"}</span>
    </div>
  );
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_INVOKE__;
  delete (window as unknown as Record<string, unknown>).__DUCKTAPE_TEST_NATIVE_EVENTS__;
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  invokeMock.mockReset();
  nativeEvents.handlers.clear();
  localStorage.clear();
  actions = null;
});

/** A stubbed node surface: /v1/status answers with `pubkey` as the node's
 *  identity, valset queries answer by variant, everything else is generic. */
const nodeFetch = (
  valset: { validators: number[][]; residents?: number[][] },
  pubkey = "ab12",
) =>
  vi.fn((url: string, init?: RequestInit) => {
    const u = String(url);
    if (u.endsWith("/v1/status")) return Promise.resolve(jsonResponse(200, status(pubkey)));
    if (u.endsWith("/v1/query")) {
      const body = JSON.parse(String(init?.body ?? "{}")) as { target?: string; query?: unknown };
      if (body.target === "valset" && body.query === "validators") {
        return Promise.resolve(jsonResponse(200, { validators: valset.validators }));
      }
      if (body.target === "valset" && body.query === "residents") {
        return Promise.resolve(jsonResponse(200, { residents: valset.residents ?? [] }));
      }
      return Promise.resolve(jsonResponse(200, { channels: [] }));
    }
    return Promise.resolve(jsonResponse(200, { channels: [] }));
  });

/** Boot the provider with `list` in the registry and no active workspace;
 *  `handlers` overlay per-command invoke behavior. Boot lands at the account
 *  Home (state.atHome) whether or not the registry has networks (epic W1) — no
 *  network is entered, and the ConnectPanel UI is rendered in the tree for the
 *  picker affordances the tests drive. */
const bootGate = async (
  list: Workspace[],
  handlers: Record<string, (args?: Record<string, unknown>) => unknown> = {},
) => {
  markNative();
  invokeMock.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    if (cmd in handlers) return Promise.resolve(handlers[cmd](args));
    switch (cmd) {
      case "workspace_list":
        return Promise.resolve(list);
      case "workspace_active":
        return Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
  });
  render(
    <DucktapeProvider>
      <ConnectPanel />
      <Probe />
    </DucktapeProvider>,
  );
  // boot settled and no network entered: the account home, registry empty or not.
  await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("true"));
};

describe("selecting a not-admitted workspace from the picker", () => {
  it("shows an error and stays on the picker instead of entering anything", async () => {
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    await bootGate([guest], {
      workspace_phase: () => ({ phase: "parked", detail: "awaiting admission" }),
    });

    await act(async () => {
      actions!.selectWorkspace("g");
    });

    await waitFor(() =>
      expect(screen.getByTestId("error").textContent).toMatch(/hasn't been admitted/i),
    );
    // never repointed the registry, never spawned, never entered a workspace.
    expect(invokeMock).not.toHaveBeenCalledWith("workspace_select", expect.anything());
    expect(screen.getByTestId("home").textContent).toBe("true");
    expect(screen.getByTestId("ws").textContent).toBe("none");
  });

  it("surfaces a fatal join as its own error, still without entering", async () => {
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    await bootGate([guest], {
      workspace_phase: () => ({ phase: "fatal", detail: "not admitted after 900 attempts" }),
    });

    await act(async () => {
      actions!.selectWorkspace("g");
    });

    await waitFor(() =>
      expect(screen.getByTestId("error").textContent).toMatch(/not admitted after/i),
    );
    expect(invokeMock).not.toHaveBeenCalledWith("workspace_select", expect.anything());
    expect(screen.getByTestId("home").textContent).toBe("true");
  });

  it("re-clicking the CURRENT not-admitted workspace still surfaces the error", async () => {
    // boot resumes a parked workspace's waiting room; opening the picker and
    // clicking that same workspace must not silently no-op (the old
    // current-id early return) — the user asked for the honest status.
    markNative();
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "workspace_list":
          return Promise.resolve([guest]);
        case "workspace_active":
          return Promise.resolve(guest);
        case "workspace_select":
          return Promise.resolve({ id: "g", httpUrl: "http://127.0.0.1:9002" });
        case "workspace_phase":
          return Promise.resolve({ phase: "parked", detail: "awaiting admission" });
        default:
          return Promise.resolve(null);
      }
    });
    // its surface never answers — boot lands in the waiting room.
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new Error("refused"))));
    render(
      <DucktapeProvider>
        <ConnectPanel />
        <Probe />
      </DucktapeProvider>,
    );
    await waitFor(() => expect(screen.getByTestId("phase").textContent).toBe("parked"));

    await act(async () => {
      actions!.newWorkspace();
      actions!.selectWorkspace("g");
    });

    await waitFor(() =>
      expect(screen.getByTestId("error").textContent).toMatch(/hasn't been admitted/i),
    );
    expect(screen.getByTestId("gate").textContent).toBe("true");
  });

  it("lets a promoted (now-member) workspace connect normally", async () => {
    const guest = workspace({
      id: "g",
      name: "Guest",
      founder: false,
      member: false,
      ports: { listen: 1, http: 9002, rpc: 3 },
    });
    await bootGate([guest], {
      workspace_phase: () => ({ phase: "promoted", detail: "validator at epoch 1" }),
      workspace_select: () => ({ id: "g", httpUrl: "http://127.0.0.1:9002" }),
    });
    // its valset carries our key — the node genuinely promoted.
    vi.stubGlobal("fetch", nodeFetch({ validators: [[0xab, 0x12]] }));

    await act(async () => {
      actions!.selectWorkspace("g");
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("workspace_select", { id: "g" });
      expect(screen.getByTestId("ws").textContent).toBe("Guest");
      expect(screen.getByTestId("gate").textContent).toBe("false");
    });
  });
});

describe("join flow", () => {
  it("keeps the waiting room up while a PARKED node serves its surface", async () => {
    // parked joiners DO serve http/rpc (newer node builds): a merely-answering
    // /v1/status must not open the console on a not-admitted workspace. Only
    // valset membership proves readiness.
    const guest = workspace({
      id: "g",
      name: "Guest",
      founder: false,
      member: false,
      pubkey: "ab12",
      ports: { listen: 1, http: 9002, rpc: 3 },
    });
    await bootGate([], {
      workspace_join: () => guest,
      workspace_select: () => ({ id: "g", httpUrl: "http://127.0.0.1:9002" }),
      workspace_phase: () => ({ phase: "parked", detail: "awaiting admission" }),
    });
    // the parked node's surface answers — but its valset does NOT contain us.
    vi.stubGlobal("fetch", nodeFetch({ validators: [[0xff, 0x99]] }));

    await act(async () => {
      actions!.joinWorkspace("Guest", "ducktape-invite-v2:blob");
    });

    // still in the waiting room — the console never opened on the parked node.
    expect(screen.getByTestId("gate").textContent).toBe("false");
    expect(screen.getByTestId("phase").textContent).toBe("parked");
    expect(screen.getByTestId("ws").textContent).toBe("Guest");
  });

  it("opens the console when the joined node has resident standing", async () => {
    const guest = workspace({
      id: "g",
      name: "Guest",
      founder: false,
      member: false,
      pubkey: "ab12",
      ports: { listen: 1, http: 9002, rpc: 3 },
    });
    await bootGate([], {
      workspace_join: () => guest,
      workspace_select: () => ({ id: "g", httpUrl: "http://127.0.0.1:9002" }),
      workspace_phase: () => ({
        phase: "synced",
        detail: "resident: pre-synced boundary 9",
      }),
    });
    vi.stubGlobal(
      "fetch",
      nodeFetch({ validators: [[0xff, 0x99]], residents: [[0xab, 0x12]] }),
    );

    await act(async () => {
      actions!.joinWorkspace("Guest", "ducktape-invite-v2:blob");
    });

    await waitFor(() => {
      expect(screen.getByTestId("phase").textContent).toBe("none");
      expect(screen.getByTestId("ws").textContent).toBe("Guest");
    });
  });

  it("holds the waiting room when the parked surface rejects reads outright", async () => {
    // the live failure signature: a parked node answers /v1/status but rejects
    // every query with `parked: not admitted yet — no state to serve`. that
    // rejection must not adopt the node, escape the waiting room, or surface
    // as an error — parked is a STEP, not a failure.
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    await bootGate([], {
      workspace_join: () => guest,
      workspace_select: () => ({ id: "g", httpUrl: "http://127.0.0.1:9002" }),
      workspace_phase: () => ({ phase: "parked", detail: "awaiting admission" }),
    });
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        String(url).endsWith("/v1/status")
          ? Promise.resolve(jsonResponse(200, status("ab12")))
          : Promise.resolve(
              jsonResponse(500, {
                error: "parked: not admitted yet — no state to serve",
              }),
            ),
      ),
    );

    await act(async () => {
      actions!.joinWorkspace("Guest", "ducktape-invite-v2:blob");
    });

    expect(screen.getByTestId("gate").textContent).toBe("false");
    expect(screen.getByTestId("phase").textContent).toBe("parked");
    expect(screen.getByTestId("error").textContent).toBe("none");
  });

  it("seeds the waiting-room phase synchronously — the console never flashes", async () => {
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    await bootGate([], {
      workspace_join: () => guest,
      workspace_select: () => ({ id: "g", httpUrl: "http://127.0.0.1:9002" }),
      // the phase read never resolves — the seeded phase must already be up.
      workspace_phase: () => new Promise(() => {}),
    });
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new Error("refused"))));

    await act(async () => {
      actions!.joinWorkspace("Guest", "ducktape-invite-v2:blob");
    });

    // the gate is down and the phase is already non-null (seeded "starting"),
    // so DucktapeConsole renders JoinProgress — never the console shell.
    expect(screen.getByTestId("gate").textContent).toBe("false");
    expect(screen.getByTestId("phase").textContent).toBe("starting");
  });
});

describe("node identity check on connect", () => {
  it("backs out to the picker when the answering node is not this workspace's", async () => {
    const team = workspace({});
    await bootGate([team], {
      workspace_select: () => ({ id: "team", httpUrl: "http://127.0.0.1:9001" }),
    });
    // something answers on the workspace's port — with a DIFFERENT identity.
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        String(url).endsWith("/v1/status")
          ? Promise.resolve(jsonResponse(200, status("ff99")))
          : Promise.resolve(jsonResponse(200, { channels: [] })),
      ),
    );

    await act(async () => {
      actions!.selectWorkspace("team");
    });

    await waitFor(() =>
      expect(screen.getByTestId("error").textContent).toMatch(/different node identity/i),
    );
    // not adopted: back on the gate rather than showing the impostor's data.
    expect(screen.getByTestId("gate").textContent).toBe("true");
    expect(screen.getByTestId("ws").textContent).toBe("none");
  });
});

describe("deleteWorkspace", () => {
  it("deletes a non-active workspace by id and only drops it from the list", async () => {
    const team = workspace({});
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    await bootGate([team, guest], {
      workspace_forget: () => null,
    });

    await act(async () => {
      actions!.deleteWorkspace("g");
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("workspace_forget", { id: "g", force: false });
      expect(screen.getByTestId("list").textContent).toBe("team");
    });
    // stays at Home; never connected anywhere as a side effect.
    expect(screen.getByTestId("home").textContent).toBe("true");
    expect(invokeMock).not.toHaveBeenCalledWith("workspace_select", expect.anything());
  });

  it("scopes the force escalation to the refused workspace, then force-deletes", async () => {
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    let refuse = true;
    await bootGate([guest], {
      workspace_forget: () => {
        if (refuse) throw new Error("start the node and finish leaving — unconfirmed");
        return null;
      },
    });

    await act(async () => {
      actions!.deleteWorkspace("g");
    });
    await waitFor(() => {
      expect(screen.getByTestId("needs-force").textContent).toBe("g");
      expect(screen.getByTestId("error").textContent).toMatch(/finish leaving/i);
    });
    expect(screen.getByTestId("list").textContent).toBe("g");

    refuse = false;
    await act(async () => {
      actions!.deleteWorkspace("g", true);
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("workspace_forget", { id: "g", force: true });
      expect(screen.getByTestId("list").textContent).toBe("");
      expect(screen.getByTestId("needs-force").textContent).toBe("none");
    });
  });

  it("deleting the ACTIVE workspace tears down and falls back to the account home", async () => {
    markNative();
    const team = workspace({});
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "workspace_list":
          return Promise.resolve([team]);
        case "workspace_active":
          return Promise.resolve(team);
        case "workspace_select":
          return Promise.resolve({ id: "team", httpUrl: "http://127.0.0.1:9001" });
        case "workspace_forget":
          return Promise.resolve(null);
        default:
          return Promise.resolve(null);
      }
    });
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        String(url).endsWith("/v1/status")
          ? Promise.resolve(jsonResponse(200, status("ab12")))
          : Promise.resolve(jsonResponse(200, { channels: [] })),
      ),
    );
    render(
      <DucktapeProvider>
        <Probe />
      </DucktapeProvider>,
    );
    await waitFor(() => expect(screen.getByTestId("ws").textContent).toBe("Team"));

    await act(async () => {
      actions!.deleteWorkspace("team");
    });

    await waitFor(() => {
      expect(screen.getByTestId("ws").textContent).toBe("none");
      expect(screen.getByTestId("home").textContent).toBe("true");
      expect(screen.getByTestId("list").textContent).toBe("");
    });
  });
});

describe("onboarding gate — delete affordance", () => {
  it("deletes a listed workspace after an in-app confirm", async () => {
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    await bootGate([guest], {
      workspace_forget: () => null,
    });
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    try {
      await act(async () => {
        fireEvent.click(screen.getByLabelText("Delete network Guest"));
      });
      const dialog = screen.getByRole("dialog", { name: /delete Guest/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      await act(async () => {
        fireEvent.click(within(dialog).getByRole("button", { name: /delete network/i }));
      });

      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("workspace_forget", { id: "g", force: false });
        expect(screen.getByTestId("list").textContent).toBe("");
      });
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("a cancelled delete dialog deletes nothing", async () => {
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    await bootGate([guest], {
      workspace_forget: () => null,
    });
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(false);

    try {
      await act(async () => {
        fireEvent.click(screen.getByLabelText("Delete network Guest"));
      });
      const dialog = screen.getByRole("dialog", { name: /delete Guest/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      await act(async () => {
        fireEvent.click(within(dialog).getByRole("button", { name: /cancel/i }));
      });

      expect(invokeMock).not.toHaveBeenCalledWith("workspace_forget", expect.anything());
      expect(screen.getByTestId("list").textContent).toBe("g");
    } finally {
      nativeConfirm.mockRestore();
    }
  });

  it("offers force delete for the workspace a refused delete flagged", async () => {
    const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
    let refuse = true;
    await bootGate([guest], {
      workspace_forget: () => {
        if (refuse) throw new Error("start the node and finish leaving — unconfirmed");
        return null;
      },
    });
    const nativeConfirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    try {
      await act(async () => {
        fireEvent.click(screen.getByLabelText("Delete network Guest"));
      });
      let dialog = screen.getByRole("dialog", { name: /delete Guest/i });
      expect(nativeConfirm).not.toHaveBeenCalled();
      await act(async () => {
        fireEvent.click(within(dialog).getByRole("button", { name: /delete network/i }));
      });
      await waitFor(() =>
        expect(screen.getByTestId("needs-force").textContent).toBe("g"),
      );

      refuse = false;
      await act(async () => {
        fireEvent.click(screen.getByLabelText("Force delete network Guest"));
      });
      dialog = screen.getByRole("dialog", { name: /force-delete Guest/i });
      await act(async () => {
        fireEvent.click(within(dialog).getByRole("button", { name: /force delete/i }));
      });
      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("workspace_forget", { id: "g", force: true });
        expect(screen.getByTestId("list").textContent).toBe("");
      });
      expect(nativeConfirm).not.toHaveBeenCalled();
    } finally {
      nativeConfirm.mockRestore();
    }
  });
});

describe("auto-bind retry on identity unlock", () => {
  // The founder-never-bound seam: the boot connect always outruns a human
  // typing a password, so on an encrypted key the connect-time auto-bind
  // deterministically short-circuits "locked" — and nothing used to retry.
  // The shell now announces `ducktape://identity-unlocked` the moment the
  // session password cache holds a verified password; the provider re-runs
  // the bind (and the parked first-run display name) against the live node.

  const boundMsg = JSON.stringify({
    bind_node: {
      authorizer: { key: [1, 2, 3], kind: "ed25519", proof: { signature: { sig: [9, 9, 9] } } },
    },
  });

  /** Boot straight into the member workspace over a node whose identity
   *  module is empty; `identity.current` plays the machine key's state and
   *  `submits` collects every parsed /v1/submit body, in order. */
  const bootConnected = async (identity: { current: unknown }) => {
    markNative();
    const submits: Array<{ target?: string; payload?: unknown; origin?: string }> = [];
    const team = workspace({});
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "workspace_list":
          return Promise.resolve([team]);
        case "workspace_active":
          return Promise.resolve(team);
        case "workspace_select":
          return Promise.resolve({ id: "team", httpUrl: "http://127.0.0.1:9001" });
        case "user_identity_state":
          return Promise.resolve(identity.current);
        case "user_sign_bind":
          return Promise.resolve(boundMsg);
        default:
          return Promise.resolve(null);
      }
    });
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string, init?: RequestInit) => {
        const u = String(url);
        if (u.endsWith("/v1/status")) return Promise.resolve(jsonResponse(200, status("ab12")));
        if (u.endsWith("/v1/submit")) {
          submits.push(JSON.parse(String(init?.body ?? "{}")));
          return Promise.resolve(
            jsonResponse(200, { height: 1, appHash: "bb".repeat(32), ops: [] }),
          );
        }
        if (u.endsWith("/v1/query")) {
          const body = JSON.parse(String(init?.body ?? "{}")) as { target?: string };
          if (body.target === "identity") return Promise.resolve(jsonResponse(200, { account: null }));
          return Promise.resolve(jsonResponse(200, { channels: [] }));
        }
        return Promise.resolve(jsonResponse(200, { channels: [] }));
      }),
    );
    render(
      <DucktapeProvider>
        <Probe />
      </DucktapeProvider>,
    );
    await waitFor(() => expect(screen.getByTestId("ws").textContent).toBe("Team"));
    // the connect-time pass has run (and, when locked, short-circuited).
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("user_identity_state"));
    return submits;
  };

  it("binds the node once the shell announces the unlocked identity", async () => {
    const identity = {
      current: { state: "locked", pubkey: "cd34", mnemonicConfirmed: false },
    };
    const submits = await bootConnected(identity);
    expect(invokeMock).not.toHaveBeenCalledWith("user_sign_bind", expect.anything());

    identity.current = { state: "unlocked", pubkey: "cd34", mnemonicConfirmed: true };
    await act(async () => {
      nativeEvents.emitTo("ducktape://identity-unlocked", null);
    });

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("user_sign_bind", {
        chainId: "team#abcd",
        nodePub: "ab12",
        nonce: 0,
      }),
    );
    await waitFor(() =>
      expect(submits).toContainEqual(
        expect.objectContaining({ target: "identity", payload: JSON.parse(boundMsg) }),
      ),
    );
  });

  it("lands the parked first-run display name with the unlock-driven bind", async () => {
    localStorage.setItem("ducktape.pendingDisplayName", "오소리");
    const identity = {
      current: { state: "locked", pubkey: "cd34", mnemonicConfirmed: false },
    };
    const submits = await bootConnected(identity);

    identity.current = { state: "unlocked", pubkey: "cd34", mnemonicConfirmed: true };
    await act(async () => {
      nativeEvents.emitTo("ducktape://identity-unlocked", null);
    });

    await waitFor(() =>
      expect(submits).toContainEqual(
        expect.objectContaining({
          target: "identity",
          payload: { set_account_name: { display_name: "오소리" } },
        }),
      ),
    );
    expect(localStorage.getItem("ducktape.pendingDisplayName")).toBeNull();
  });

  it("no-ops safely when the unlock lands before any workspace is connected", async () => {
    await bootGate([workspace({})], {
      user_identity_state: () => ({ state: "unlocked", pubkey: "cd34", mnemonicConfirmed: true }),
    });

    await act(async () => {
      nativeEvents.emitTo("ducktape://identity-unlocked", null);
    });

    expect(invokeMock).not.toHaveBeenCalledWith("user_sign_bind", expect.anything());
    expect(screen.getByTestId("error").textContent).toBe("none");
  });
});
