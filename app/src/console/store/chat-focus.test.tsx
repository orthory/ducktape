// Jump-to-message wiring (focusMessage / clearChatFocus): a tag or search hit
// enters its channel on the chat screen, drops any tag filter, and latches the
// one-shot chatFocusSeq for ChatView to scroll+flash and then consume.
// Same mocked-invoke + stubbed-node harness as home-routing.test.tsx.

import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";
import type { Workspace } from "../../domain/workspace-client";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const status = (publicKey = "ab12") => ({
  version: "0.1.0",
  appHash: "aa".repeat(32),
  height: 0,
  modules: [],
  publicKey,
});

const jsonResponse = (code: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status: code,
    headers: { "content-type": "application/json" },
  });

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

/** A stubbed node: status answers with a pubkey, valset queries answer by
 *  variant, chat's messages_latest answers empty — so refresh() lands connected
 *  and enterChannel's slice load resolves cleanly (no fail() noise). */
const nodeFetch = () =>
  vi.fn((url: string, init?: RequestInit) => {
    const u = String(url);
    if (u.endsWith("/v1/status")) return Promise.resolve(jsonResponse(200, status()));
    if (u.endsWith("/v1/query")) {
      const body = JSON.parse(String(init?.body ?? "{}")) as { target?: string; query?: unknown };
      if (body.target === "valset" && body.query === "validators") {
        return Promise.resolve(jsonResponse(200, { validators: [[0xab, 0x12]] }));
      }
      if (body.target === "valset" && body.query === "residents") {
        return Promise.resolve(jsonResponse(200, { residents: [] }));
      }
      if (body.query && typeof body.query === "object" && "messages_latest" in body.query) {
        return Promise.resolve(jsonResponse(200, { messages: [] }));
      }
      return Promise.resolve(jsonResponse(200, { channels: [] }));
    }
    return Promise.resolve(jsonResponse(200, { channels: [] }));
  });

const workspace = (over: Partial<Workspace> = {}): Workspace => ({
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
      <span data-testid="ws">{state.workspace?.name ?? "none"}</span>
      <span data-testid="nodeUrl">{state.nodeUrl ?? "none"}</span>
      <span data-testid="screen">{state.screen}</span>
      <span data-testid="channel">{state.activeChannel ?? "none"}</span>
      <span data-testid="focus">{String(state.chatFocusSeq)}</span>
      <span data-testid="tag">{state.tagFilter?.tag ?? "none"}</span>
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
  window.history.replaceState(null, "");
});

const bootConnected = async () => {
  markTauri();
  const team = workspace();
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
  vi.stubGlobal("fetch", nodeFetch());
  await act(async () => {
    render(
      <DucktapeProvider>
        <Probe />
      </DucktapeProvider>,
    );
  });
  await waitFor(() => {
    expect(screen.getByTestId("ws").textContent).toBe("Team");
    expect(screen.getByTestId("nodeUrl").textContent).not.toBe("none");
  });
};

describe("focusMessage", () => {
  it("enters the channel on chat, drops the tag filter, and latches the seq", async () => {
    await bootConnected();

    // a tag filter is up; the jump must clear it (enterChannel semantics).
    await act(async () => {
      actions!.setTagFilter("bug");
    });
    expect(screen.getByTestId("tag").textContent).toBe("bug");

    await act(async () => {
      actions!.focusMessage("general", 42);
    });

    expect(screen.getByTestId("screen").textContent).toBe("chat");
    expect(screen.getByTestId("channel").textContent).toBe("general");
    expect(screen.getByTestId("focus").textContent).toBe("42");
    expect(screen.getByTestId("tag").textContent).toBe("none");
  });

  it("clearChatFocus consumes the one-shot", async () => {
    await bootConnected();

    await act(async () => {
      actions!.focusMessage("general", 7);
    });
    expect(screen.getByTestId("focus").textContent).toBe("7");

    await act(async () => {
      actions!.clearChatFocus();
    });
    expect(screen.getByTestId("focus").textContent).toBe("null");
  });

  it("switching channels drops a focus ChatView never consumed", async () => {
    await bootConnected();

    // jump to a message, then leave for another channel via the rail before
    // ChatView acted on the seq — the stale focus must not ride along and
    // flash whatever message carries seq 7 over there.
    await act(async () => {
      actions!.focusMessage("general", 7);
    });
    expect(screen.getByTestId("focus").textContent).toBe("7");

    await act(async () => {
      actions!.selectChannel("random");
    });
    expect(screen.getByTestId("channel").textContent).toBe("random");
    expect(screen.getByTestId("focus").textContent).toBe("null");
  });
});
