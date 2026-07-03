// Desktop onboarding contract: with no active workspace the gate is raised;
// founding connects; joining parks and surfaces the phase. Drives the provider
// over a mocked Tauri `invoke` + a stubbed node surface.

import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";
import { LIVE_JOIN_SUPPORTED } from "../../domain/workspace-client";
import type { Workspace } from "../../domain/workspace-client";
import { OnboardingGate } from "../views/onboarding/OnboardingGate";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const status = { version: "0.1.0", appHash: "aa".repeat(32), height: 0, modules: [] };

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
    </div>
  );
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  invokeMock.mockReset();
  actions = null;
});

describe("desktop onboarding", () => {
  it("first run with no workspace raises the gate", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) =>
      cmd === "workspace_list" ? Promise.resolve([]) : Promise.resolve(null),
    );

    render(
      <DucktapeProvider>
        <Probe />
      </DucktapeProvider>,
    );

    await waitFor(() => expect(screen.getByTestId("gate").textContent).toBe("true"));
  });

  it("createWorkspace founds a network and connects", async () => {
    markTauri();
    const team = workspace({});
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "workspace_list":
          return Promise.resolve([]);
        case "workspace_active":
          return Promise.resolve(null);
        case "workspace_create":
          return Promise.resolve(team);
        case "workspace_select":
          return Promise.resolve({ id: "team", httpUrl: "http://127.0.0.1:9001" });
        default:
          return Promise.resolve(null);
      }
    });
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        String(url).endsWith("/v1/status")
          ? Promise.resolve(jsonResponse(200, status))
          : Promise.resolve(jsonResponse(200, { Channels: [] })),
      ),
    );

    render(
      <DucktapeProvider>
        <Probe />
      </DucktapeProvider>,
    );
    await waitFor(() => expect(screen.getByTestId("gate").textContent).toBe("true"));

    await act(async () => {
      actions!.createWorkspace("Team");
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("workspace_create", { name: "Team" });
      expect(screen.getByTestId("ws").textContent).toBe("Team");
      expect(screen.getByTestId("gate").textContent).toBe("false");
    });
  });

  it("joinWorkspace parks and surfaces the phase until the node answers", async () => {
    vi.useFakeTimers();
    try {
      markTauri();
      const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
      invokeMock.mockImplementation((cmd: string) => {
        switch (cmd) {
          case "workspace_list":
            return Promise.resolve([]);
          case "workspace_active":
            return Promise.resolve(null);
          case "workspace_join":
            return Promise.resolve(guest);
          case "workspace_select":
            return Promise.resolve({ id: "g", httpUrl: "http://127.0.0.1:9002" });
          case "workspace_phase":
            return Promise.resolve({ phase: "parked", detail: "awaiting admission" });
          default:
            return Promise.resolve(null);
        }
      });
      // parked node: its surface never answers, so the phase poll drives the ui.
      vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new Error("refused"))));

      render(
        <DucktapeProvider>
          <Probe />
        </DucktapeProvider>,
      );
      await act(async () => {}); // flush boot → gate
      expect(screen.getByTestId("gate").textContent).toBe("true");

      await act(async () => {
        actions!.joinWorkspace("Guest", "ducktape-invite-v1:blob");
      });

      expect(invokeMock).toHaveBeenCalledWith("workspace_join", {
        name: "Guest",
        blob: "ducktape-invite-v1:blob",
      });
      expect(screen.getByTestId("ws").textContent).toBe("Guest");
      expect(screen.getByTestId("phase").textContent).toBe("parked");
    } finally {
      vi.useRealTimers();
    }
  });
});

// Live join (network-shape admission) landed at the node layer in PR #77, so the
// gate's Join tab is reachable, not the disabled "temporarily unavailable" note.
// This is the regression guard for LIVE_JOIN_SUPPORTED being flipped back off.
describe("onboarding gate — live join UI", () => {
  it("exposes the Join form and dispatches a join from it", async () => {
    expect(LIVE_JOIN_SUPPORTED).toBe(true);
    vi.useFakeTimers();
    try {
      markTauri();
      const guest = workspace({ id: "g", name: "Guest", founder: false, member: false });
      invokeMock.mockImplementation((cmd: string) => {
        switch (cmd) {
          case "workspace_list":
            return Promise.resolve([]);
          case "workspace_active":
            return Promise.resolve(null);
          case "workspace_join":
            return Promise.resolve(guest);
          case "workspace_select":
            return Promise.resolve({ id: "g", httpUrl: "http://127.0.0.1:9002" });
          case "workspace_phase":
            return Promise.resolve({ phase: "parked", detail: "awaiting admission" });
          default:
            return Promise.resolve(null);
        }
      });
      // a parked joiner's surface never answers; keep connect from throwing loudly.
      vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new Error("refused"))));

      render(
        <DucktapeProvider>
          <OnboardingGate />
        </DucktapeProvider>,
      );
      await act(async () => {}); // flush boot

      // switch to the Join tab (the "Join" tab button, not the "Join workspace" submit)
      fireEvent.click(screen.getByText("Join"));

      // the join form is live — no disabled note, the invite-blob field is present
      expect(screen.queryByText(/temporarily unavailable/i)).toBeNull();
      fireEvent.change(screen.getByPlaceholderText("Workspace name"), {
        target: { value: "Guest" },
      });
      fireEvent.change(screen.getByPlaceholderText(/Paste invite blob/i), {
        target: { value: "ducktape-invite-v1:blob" },
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Join workspace"));
      });

      expect(invokeMock).toHaveBeenCalledWith("workspace_join", {
        name: "Guest",
        blob: "ducktape-invite-v1:blob",
      });
    } finally {
      vi.useRealTimers();
    }
  });
});
