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
  navStackAfter,
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
  forgeMessageId: null,
  forgeMessageSeq: null,
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
  it("round-trips a stamped snapshot with its stack position", () => {
    const s = snap({
      channel: "general",
      forgeItem: 7,
      forgeMessageId: "m7",
      forgeMessageSeq: 4,
    });
    expect(readNavEntry(stampNav(s, 3))).toEqual({ snap: s, index: 3 });
  });

  it("reads pre-anchor history entries with empty Forge message focus", () => {
    const old = stampNav(snap(), 1) as unknown as Record<string, unknown>;
    delete old.forgeMessageId;
    delete old.forgeMessageSeq;
    expect(readNavEntry(old)?.snap).toMatchObject({
      forgeMessageId: null,
      forgeMessageSeq: null,
    });
  });

  it("rejects foreign or malformed history state", () => {
    expect(readNavEntry(null)).toBeNull();
    expect(readNavEntry("scroll-pos")).toBeNull();
    expect(readNavEntry({ k: "someone-else" })).toBeNull();
    expect(readNavEntry({ ...stampNav(snap(), 0), viewMode: "root" })).toBeNull();
    expect(readNavEntry({ ...stampNav(snap(), 0), forgeItem: "7" })).toBeNull();
    expect(readNavEntry({ ...stampNav(snap(), 0), i: "3" })).toBeNull();
  });
});

describe("navStackAfter", () => {
  it("a push lands on `at` and truncates the forward tail", () => {
    expect(navStackAfter("push", 1, { index: 0, count: 1 })).toEqual({ index: 1, count: 2 });
    // pushing from mid-stack discards the entries beyond the new one
    expect(navStackAfter("push", 2, { index: 1, count: 5 })).toEqual({ index: 2, count: 3 });
  });

  it("a replace stays in place and can only reveal a deeper stack", () => {
    expect(navStackAfter("replace", 1, { index: 1, count: 3 })).toEqual({ index: 1, count: 3 });
    // a reload restoring a mid-stack entry boots with count 1 — the entry's
    // own position proves the stack is at least that deep.
    expect(navStackAfter("replace", 4, { index: 0, count: 1 })).toEqual({ index: 4, count: 5 });
  });

  it("a traversal moves within the stack without shrinking it", () => {
    expect(navStackAfter("traverse", 0, { index: 2, count: 3 })).toEqual({ index: 0, count: 3 });
    expect(navStackAfter("traverse", 2, { index: 0, count: 3 })).toEqual({ index: 2, count: 3 });
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
      <span data-testid="nav">{`${state.nav.index}/${state.nav.count}`}</span>
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
    expect(entry?.snap.screen).toBe("chat");
    expect(entry?.snap.channel).toBe("general");
    // hydration replaced the boot entry rather than pushing new ones.
    expect(window.history.length).toBe(lengthBefore);
    // ...so the stack position still says "nowhere to go".
    expect(screen.getByTestId("nav").textContent).toBe("0/1");
  });

  it("screen switches push entries; back/forward walk them and restore the rail", async () => {
    // status = an operator-section screen; this harness's managed workspace
    // passes the node-control gate, so the rail genuinely flips (ADR A5/A6).
    await bootShell();
    const lengthBefore = window.history.length;

    await act(async () => {
      actions!.setScreen("status");
    });
    expect(screen.getByTestId("viewMode").textContent).toBe("operator");
    expect(window.history.length).toBe(lengthBefore + 1);
    expect(readNavEntry(window.history.state)?.snap.screen).toBe("status");
    expect(screen.getByTestId("nav").textContent).toBe("1/2");

    await traverse(() => window.history.back());
    await waitFor(() => expect(screen.getByTestId("screen").textContent).toBe("chat"));
    expect(screen.getByTestId("viewMode").textContent).toBe("user");
    // walked back within the stack — forward stays available.
    expect(screen.getByTestId("nav").textContent).toBe("0/2");

    await traverse(() => window.history.forward());
    await waitFor(() => expect(screen.getByTestId("screen").textContent).toBe("status"));
    expect(screen.getByTestId("viewMode").textContent).toBe("operator");
    expect(screen.getByTestId("nav").textContent).toBe("1/2");
  });

  it("a push from mid-stack truncates the forward tail", async () => {
    await bootShell();

    await act(async () => {
      actions!.setScreen("members");
    });
    await traverse(() => window.history.back());
    await waitFor(() => expect(screen.getByTestId("nav").textContent).toBe("0/2"));

    await act(async () => {
      actions!.setScreen("agent");
    });
    // the members entry ahead is gone — forward must disable again.
    expect(screen.getByTestId("nav").textContent).toBe("1/2");
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
    expect(readNavEntry(window.history.state)?.snap.atHome).toBe(true);

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
            1,
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
    expect(readNavEntry(window.history.state)?.snap.agent).toBe("agent-1");

    // the view consumes the hand-off — the entry must keep it.
    await act(async () => {
      actions!.clearAgentFocus();
    });
    expect(screen.getByTestId("agentFocus").textContent).toBe("none");
    expect(readNavEntry(window.history.state)?.snap.agent).toBe("agent-1");
  });

  it("traversal is ignored while the account home owns the window", async () => {
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
    // epic W1: first run lands on the account home (atHome), not a gate — with
    // no node behind it, a traversal to a shell screen must be ignored.
    await waitFor(() => expect(screen.getByTestId("home").textContent).toBe("true"));

    await act(async () => {
      window.dispatchEvent(
        new PopStateEvent("popstate", {
          state: stampNav(snap({ screen: "members", viewMode: "operator", scope: "session" }), 1),
        }),
      );
    });

    expect(screen.getByTestId("screen").textContent).toBe("chat");
    expect(screen.getByTestId("home").textContent).toBe("true");
  });
});
