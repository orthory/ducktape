// Browser-history navigation (nav-history.ts + applyNavSnapshot + the
// provider's 5d/5e effects):
//   - pure pieces: the push/replace/none transition decision, one-shot focus
//     latching, entry (de)serialization hardening;
//   - the mule, end to end: before this feature the webview session held ONE
//     inert history entry — now surface moves push entries, boot hydration
//     replaces (no phantom entries), traversal restores the surface AND
//     re-fetches the restored target's data from the node (recent data, not a
//     stale copy), cross-scope entries never apply another workspace's
//     selections, and gated bodies (onboarding) ignore traversal.
// Same mocked-invoke + stubbed-node harness as home-routing.test.tsx.

import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";
import {
  latchOneShots,
  navTransition,
  readNavEntry,
  stampNav,
} from "./nav-history";
import type { NavSnapshot } from "./nav-history";
import type { Channel } from "../../domain/chat-client";
import type { Workspace } from "../../domain/workspace-client";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

// ── Pure helpers ────────────────────────────────────────

const snap = (over: Partial<NavSnapshot> = {}): NavSnapshot => ({
  scope: "workspace:team",
  atHome: false,
  screen: "chat",
  viewMode: "user",
  channel: null,
  page: null,
  forgeRepo: null,
  forgeItem: null,
  explorer: null,
  agent: null,
  member: null,
  ...over,
});

describe("navTransition", () => {
  it("stamps the very first entry in place", () => {
    expect(navTransition(snap(), null)).toBe("replace");
  });

  it("is quiet when nothing nav-relevant changed", () => {
    expect(navTransition(snap(), snap())).toBe("none");
  });

  it("pushes on a surface move (screen / rail / home)", () => {
    expect(navTransition(snap({ screen: "members", viewMode: "operator" }), snap())).toBe("push");
    expect(navTransition(snap({ atHome: true }), snap())).toBe("push");
  });

  it("a scope change alone re-stamps in place — boot adoption is not a nav", () => {
    expect(navTransition(snap({ scope: "workspace:other" }), snap())).toBe("replace");
  });

  it("replaces when boot hydration fills an empty selection slot", () => {
    expect(navTransition(snap({ channel: "general" }), snap())).toBe("replace");
  });

  it("pushes when a selection moves between values", () => {
    expect(
      navTransition(snap({ channel: "dev" }), snap({ channel: "general" })),
    ).toBe("push");
  });
});

describe("latchOneShots", () => {
  it("inherits a consumed one-shot focus while the visit lasts", () => {
    const entry = snap({ screen: "agent", agent: "agent-1" });
    const afterConsume = snap({ screen: "agent", agent: null });
    expect(latchOneShots(afterConsume, entry).agent).toBe("agent-1");
  });

  it("does not latch across a screen or scope boundary", () => {
    const entry = snap({ screen: "agent", agent: "agent-1" });
    expect(latchOneShots(snap({ screen: "chat" }), entry).agent).toBeNull();
    expect(
      latchOneShots(snap({ screen: "agent", scope: "workspace:other" }), entry).agent,
    ).toBeNull();
  });
});

describe("readNavEntry", () => {
  it("round-trips a stamped snapshot", () => {
    const s = snap({ channel: "general", forgeItem: 7 });
    expect(readNavEntry(stampNav(s))).toEqual(s);
  });

  it("rejects foreign or malformed history state", () => {
    expect(readNavEntry(null)).toBeNull();
    expect(readNavEntry("scroll-pos")).toBeNull();
    expect(readNavEntry({ k: "someone-else" })).toBeNull();
    expect(readNavEntry({ ...stampNav(snap()), viewMode: "root" })).toBeNull();
    expect(readNavEntry({ ...stampNav(snap()), forgeItem: "7" })).toBeNull();
  });
});

// ── Provider harness (home-routing.test.tsx's, plus real chat channels) ──

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

const channel = (id: string): Channel => ({
  id,
  name: id,
  created_at: 0,
  head_seq: 0,
  post_policy: "open",
  hooks: [],
  pinned: [],
});

/** The stubbed node answers chat with TWO real channels so hydration picks an
 *  active one and traversal has somewhere to go back to. */
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
      if (body.target === "chat" && body.query === "channels") {
        return Promise.resolve(jsonResponse(200, { channels: [channel("general"), channel("dev")] }));
      }
      if (
        body.target === "chat" &&
        typeof body.query === "object" &&
        body.query !== null &&
        "messages_latest" in body.query
      ) {
        return Promise.resolve(jsonResponse(200, { messages: [] }));
      }
      // the no-catch slice fetchers (agents / runs / forge head) need their
      // exact reply variant or the whole boot snapshot aborts.
      if (body.target === "agent") return Promise.resolve(jsonResponse(200, { agents: [] }));
      if (body.target === "runs") {
        return Promise.resolve(
          jsonResponse(200, body.query === "watches" ? { watches: [] } : { pending_runs: [] }),
        );
      }
      if (body.target === "forge") return Promise.resolve(jsonResponse(200, { head: null }));
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
      <span data-testid="screen">{state.screen}</span>
      <span data-testid="viewMode">{state.viewMode}</span>
      <span data-testid="home">{String(state.atHome)}</span>
      <span data-testid="gate">{String(state.needsOnboarding)}</span>
      <span data-testid="channel">{state.activeChannel ?? "none"}</span>
      <span data-testid="agentFocus">{state.agentFocus ?? "none"}</span>
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
  // jsdom's session history persists across tests in this file — park the
  // shared top entry on a null state so the next boot's readNavEntry sees a
  // clean slate rather than a previous test's entry.
  window.history.replaceState(null, "");
});

const bootShell = async (fetchStub = nodeFetch()) => {
  const team = workspace({});
  markTauri();
  vi.stubGlobal("fetch", fetchStub);
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
  await act(async () => {
    render(
      <DucktapeProvider>
        <Probe />
      </DucktapeProvider>,
    );
  });
  // settled: hydration picked the first channel — the entry is stamped.
  await waitFor(() => expect(screen.getByTestId("channel").textContent).toBe("general"));
  return fetchStub;
};

/** jsdom performs back()/forward() traversal (and its popstate dispatch) on a
 *  queued task — flush it inside act so the store update lands deterministically. */
const traverse = async (go: () => void) => {
  await act(async () => {
    go();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
};

const countMessageFetches = (fetchStub: ReturnType<typeof nodeFetch>, channelId: string) =>
  fetchStub.mock.calls.filter(([, init]) => {
    const body = String((init as RequestInit | undefined)?.body ?? "");
    return body.includes("messages_latest") && body.includes(channelId);
  }).length;

describe("browser back/forward (provider integration)", () => {
  it("boot stamps ONE entry in place — no phantom entries to walk back through", async () => {
    const lengthBefore = window.history.length;
    await bootShell();
    const entry = readNavEntry(window.history.state);
    expect(entry).not.toBeNull();
    expect(entry?.screen).toBe("chat");
    expect(entry?.channel).toBe("general");
    // hydration replaced the boot entry rather than pushing new ones.
    expect(window.history.length).toBe(lengthBefore);
  });

  it("screen switches push entries; back/forward walk them and restore the rail", async () => {
    await bootShell();
    const lengthBefore = window.history.length;

    await act(async () => {
      actions!.setScreen("members");
    });
    expect(screen.getByTestId("viewMode").textContent).toBe("operator");
    expect(window.history.length).toBe(lengthBefore + 1);
    expect(readNavEntry(window.history.state)?.screen).toBe("members");

    await traverse(() => window.history.back());
    await waitFor(() => expect(screen.getByTestId("screen").textContent).toBe("chat"));
    expect(screen.getByTestId("viewMode").textContent).toBe("user");

    await traverse(() => window.history.forward());
    await waitFor(() => expect(screen.getByTestId("screen").textContent).toBe("members"));
    expect(screen.getByTestId("viewMode").textContent).toBe("operator");
  });

  it("back into a channel re-enters it AND re-fetches its recent messages", async () => {
    const fetchStub = await bootShell();

    await act(async () => {
      actions!.selectChannel("dev");
    });
    await waitFor(() => expect(screen.getByTestId("channel").textContent).toBe("dev"));

    const generalFetches = countMessageFetches(fetchStub, "general");
    await traverse(() => window.history.back());
    await waitFor(() => expect(screen.getByTestId("channel").textContent).toBe("general"));
    // rehydrated, not restored from memory: a fresh messages_latest round-trip.
    expect(countMessageFetches(fetchStub, "general")).toBe(generalFetches + 1);
  });

  it("Home is an entry: back from Home lands on the shell again", async () => {
    await bootShell();

    await act(async () => {
      actions!.goHome();
    });
    expect(screen.getByTestId("home").textContent).toBe("true");
    expect(readNavEntry(window.history.state)?.atHome).toBe(true);

    await traverse(() => window.history.back());
    await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("false"));
    expect(screen.getByTestId("screen").textContent).toBe("chat");
  });

  it("a cross-scope entry restores the surface but never another workspace's selections", async () => {
    await bootShell();

    await act(async () => {
      window.dispatchEvent(
        new PopStateEvent("popstate", {
          state: stampNav(
            snap({
              scope: "workspace:other",
              screen: "members",
              viewMode: "operator",
              channel: "dev",
            }),
          ),
        }),
      );
    });

    await waitFor(() => expect(screen.getByTestId("screen").textContent).toBe("members"));
    // the foreign scope's channel id was NOT applied.
    expect(screen.getByTestId("channel").textContent).toBe("general");
  });

  it("consuming a one-shot focus keeps the entry restorable (latch)", async () => {
    await bootShell();

    await act(async () => {
      actions!.openAgent("agent-1");
    });
    expect(readNavEntry(window.history.state)?.agent).toBe("agent-1");

    // the view consumes the hand-off — the entry must keep it.
    await act(async () => {
      actions!.clearAgentFocus();
    });
    expect(screen.getByTestId("agentFocus").textContent).toBe("none");
    expect(readNavEntry(window.history.state)?.agent).toBe("agent-1");
  });

  it("traversal is ignored while onboarding gates the window", async () => {
    markTauri();
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "workspace_list":
          return Promise.resolve([]);
        case "workspace_active":
          return Promise.resolve(null);
        default:
          return Promise.resolve(null);
      }
    });
    await act(async () => {
      render(
        <DucktapeProvider>
          <Probe />
        </DucktapeProvider>,
      );
    });
    await waitFor(() => expect(screen.getByTestId("gate").textContent).toBe("true"));

    await act(async () => {
      window.dispatchEvent(
        new PopStateEvent("popstate", {
          state: stampNav(snap({ screen: "members", viewMode: "operator", scope: "session" })),
        }),
      );
    });

    expect(screen.getByTestId("screen").textContent).toBe("chat");
    expect(screen.getByTestId("gate").textContent).toBe("true");
  });
});
