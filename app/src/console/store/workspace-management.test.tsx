// Workspace management contract, driven over the mocked Tauri `invoke` + a
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
import { OnboardingGate } from "../views/onboarding/OnboardingGate";

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
      <span data-testid="ws">{state.workspace?.name ?? "none"}</span>
      <span data-testid="phase">{state.onboardingPhase?.phase ?? "none"}</span>
      <span data-testid="error">{state.error ?? "none"}</span>
      <span data-testid="list">{state.workspaces.map((w) => w.id).join(",")}</span>
      <span data-testid="needs-force">{state.deleteNeedsForce ?? "none"}</span>
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

/** A stubbed node surface: /v1/status answers with `pubkey` as the node's
 *  identity, the valset query answers `valset`, everything else is generic. */
const nodeFetch = (valset: { validators: number[][] }, pubkey = "ab12") =>
  vi.fn((url: string, init?: RequestInit) => {
    const u = String(url);
    if (u.endsWith("/v1/status")) return Promise.resolve(jsonResponse(200, status(pubkey)));
    if (u.endsWith("/v1/query")) {
      const body = JSON.parse(String(init?.body ?? "{}")) as { target?: string };
      if (body.target === "valset") return Promise.resolve(jsonResponse(200, valset));
      return Promise.resolve(jsonResponse(200, { channels: [] }));
    }
    return Promise.resolve(jsonResponse(200, { channels: [] }));
  });

/** Boot the provider to the raised gate with `list` in the registry and no
 *  active workspace; `handlers` overlay per-command invoke behavior. */
const bootGate = async (
  list: Workspace[],
  handlers: Record<string, (args?: Record<string, unknown>) => unknown> = {},
) => {
  markTauri();
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
      <OnboardingGate />
      <Probe />
    </DucktapeProvider>,
  );
  await waitFor(() => expect(screen.getByTestId("gate").textContent).toBe("true"));
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
    // never repointed the registry, never spawned, never left the picker.
    expect(invokeMock).not.toHaveBeenCalledWith("workspace_select", expect.anything());
    expect(screen.getByTestId("gate").textContent).toBe("true");
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
    expect(screen.getByTestId("gate").textContent).toBe("true");
  });

  it("re-clicking the CURRENT not-admitted workspace still surfaces the error", async () => {
    // boot resumes a parked workspace's waiting room; opening the picker and
    // clicking that same workspace must not silently no-op (the old
    // current-id early return) — the user asked for the honest status.
    markTauri();
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
        <OnboardingGate />
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
    // stays on the picker; never connected anywhere as a side effect.
    expect(screen.getByTestId("gate").textContent).toBe("true");
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

  it("deleting the ACTIVE workspace tears down and falls back to the gate", async () => {
    markTauri();
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
      expect(screen.getByTestId("gate").textContent).toBe("true");
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
        fireEvent.click(screen.getByLabelText("Delete workspace Guest"));
      });
      const dialog = screen.getByRole("dialog", { name: /delete Guest/i });
      expect(nativeConfirm).not.toHaveBeenCalled();

      await act(async () => {
        fireEvent.click(within(dialog).getByRole("button", { name: /delete workspace/i }));
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
        fireEvent.click(screen.getByLabelText("Delete workspace Guest"));
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
        fireEvent.click(screen.getByLabelText("Delete workspace Guest"));
      });
      let dialog = screen.getByRole("dialog", { name: /delete Guest/i });
      expect(nativeConfirm).not.toHaveBeenCalled();
      await act(async () => {
        fireEvent.click(within(dialog).getByRole("button", { name: /delete workspace/i }));
      });
      await waitFor(() =>
        expect(screen.getByTestId("needs-force").textContent).toBe("g"),
      );

      refuse = false;
      await act(async () => {
        fireEvent.click(screen.getByLabelText("Force delete workspace Guest"));
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
