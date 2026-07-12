// The huddle session lifecycle against the consensus roster, driven through the
// full provider over a fake transport with the call session stubbed at its
// module seam (createCallSession). Covers the three behaviors the live QA pass
// found missing:
//   1. reconcile — a live session whose finalized roster no longer carries us
//      (swept, or another client of this identity left) ends its media and
//      shows the "removed" error instead of a zombie "Connecting…" card;
//   2. reconnect — an unexpected close of a LIVE session re-establishes media
//      ONCE (status "reconnecting", membership kept, no leave submitted); a
//      second close inside the damp window fails honestly with the error card;
//   3. media note — a failed camera acquire surfaces a transient note.

import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { keyHex } from "../../domain/chat-client";
import type { CallEvent, CallSession } from "../../domain/call-session";
import type {
  NodeTransport,
  StreamSignal,
  TopicHandlers,
} from "../../domain/transport";
import type { EventFrame } from "../../domain/stream";
import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";
import type { ConsoleState } from "./state";

// ── the call-session stub (module seam) ─────────────────

interface StubSession {
  session: CallSession;
  onEvent: (e: CallEvent) => void;
  stop: ReturnType<typeof vi.fn>;
}
const stubs: StubSession[] = [];

vi.mock("../../domain/call-session", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../domain/call-session")>();
  return {
    ...actual,
    createCallSession: vi.fn((onEvent: (e: CallEvent) => void): CallSession => {
      const stop = vi.fn();
      const session: CallSession = {
        start: vi.fn(),
        setRecipients: vi.fn(),
        setMuted: vi.fn(),
        setCamera: vi.fn(),
        setScreenShare: vi.fn(),
        setDevices: vi.fn(),
        bindTile: vi.fn(),
        bindPreview: vi.fn(),
        stop,
      };
      stubs.push({ session, onEvent, stop });
      return session;
    }),
  };
});

// connectRemote must hand back OUR fake transport so the provider gains a
// nodeUrl (joinHuddle requires one) without dialing anything real.
let activeTransport: NodeTransport | null = null;
vi.mock("../../domain/node-bootstrap", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../domain/node-bootstrap")>();
  return {
    ...actual,
    connectRemote: vi.fn((url: string) => ({
      // a FRESH object (same method refs) — the provider's hydrate effect is
      // keyed on transport identity, and re-dialing must re-hydrate.
      transport: { ...activeTransport! },
      url,
      managed: false,
    })),
  };
});

// ── the fake node ───────────────────────────────────────

const SELF_NODE = [9];
const SELF_HEX = keyHex(SELF_NODE);
const member = (user: string, node: number[]) => ({
  user: Array.from(new TextEncoder().encode(user)),
  node,
  joined_at: 1,
});

const makeFakeNode = () => {
  const topicHandlers = new Set<TopicHandlers>();
  const streamListeners = new Set<(signal: StreamSignal) => void>();
  // The channel's huddle roster — tests mutate this, then finalize a block so
  // the provider re-queries and the store reconciles against it. The scoped
  // stream hydration diffs module roots between statuses, so each finalize must
  // move the chat root (and the height) or nothing re-queries.
  const state = { huddle: [] as ReturnType<typeof member>[], height: 1 };
  const transport: NodeTransport = {
    // A join records the member (like the real chain), so post-join refreshes
    // carry us — the reconcile must only fire on a roster that DROPPED us.
    submit: vi.fn((target: string, payload: unknown) => {
      const p = payload as {
        join_huddle?: { channel_id: string; node: number[] };
        leave_huddle?: { channel_id: string };
      };
      if (target === "chat" && p.join_huddle) {
        state.huddle = [...state.huddle, member("me", p.join_huddle.node)];
      }
      return Promise.resolve({ height: 2, appHash: "bb".repeat(32) });
    }),
    query: vi.fn((target: string, query: unknown) => {
      if (target === "chat" && query === "channels") {
        return Promise.resolve({
          channels: [
            {
              id: "general",
              name: "General",
              created_at: 1,
              head_seq: 0,
              post_policy: "open",
              hooks: [],
              pinned: [],
              huddle: [...state.huddle],
            },
          ],
        });
      }
      const latest = (query as { messages_latest?: { channel_id: string } }).messages_latest;
      if (target === "chat" && latest) return Promise.resolve({ messages: [] });
      if (target === "chat") {
        return Promise.resolve({ thread: null });
      }
      if (target === "runs" && query === "watches") return Promise.resolve({ watches: [] });
      if (target === "runs") return Promise.resolve({ pending_runs: [] });
      if (target === "agent") return Promise.resolve({ agents: [] });
      if (target === "forge") return Promise.resolve({ head: null });
      if (target === "identity") return Promise.resolve({ users: [] });
      if (target === "valset") {
        if (query === "residents") return Promise.resolve({ residents: [] });
        return Promise.resolve({ validators: [SELF_NODE] });
      }
      return Promise.resolve({});
    }),
    view: vi.fn().mockResolvedValue({ hits: [] }),
    putBlob: vi.fn().mockResolvedValue("ab".repeat(32)),
    getBlob: vi.fn().mockResolvedValue(new Uint8Array()),
    filesStage: vi.fn(),
    filesCommit: vi.fn(),
    filesStat: vi.fn(),
    filesLs: vi.fn(),
    filesRead: vi.fn(),
    filesHistory: vi.fn(),
    status: vi.fn(() =>
      Promise.resolve({
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: state.height,
        publicKey: SELF_HEX,
        modules: [{ id: "chat", root: String(state.height).padStart(64, "c") }],
      }),
    ),
    subscribe: vi.fn((_topics: string[], handlers: TopicHandlers) => {
      topicHandlers.add(handlers);
      return () => topicHandlers.delete(handlers);
    }),
    onStream: vi.fn((listener: (signal: StreamSignal) => void) => {
      streamListeners.add(listener);
      return () => streamListeners.delete(listener);
    }),
    blocks: vi.fn().mockResolvedValue([]),
  };
  const finalize = () => {
    state.height += 1;
    const frame: EventFrame = {
      type: "event",
      topic: "module:chat",
      cursor: String(state.height),
      op: {
        height: state.height,
        seq: 0,
        time: Date.now(),
        origin: { kind: "system" },
      },
    };
    topicHandlers.forEach((handlers) => handlers.onEvent?.(frame));
    streamListeners.forEach((notify) =>
      notify({
        kind: "heartbeat",
        frame: {
          type: "heartbeat",
          height: state.height,
          appHash: "cc".repeat(32),
          timeMs: Date.now(),
          intervalMs: 3_000,
        },
      }),
    );
  };
  return { transport, finalize, state };
};

// ── harness ─────────────────────────────────────────────

let capturedActions: ConsoleActions | null = null;
let capturedState: ConsoleState | null = null;

function Probe() {
  const { state, actions } = useDucktape();
  capturedActions = actions;
  capturedState = state;
  return null;
}

const leaveSubmits = (transport: NodeTransport): number =>
  vi
    .mocked(transport.submit)
    .mock.calls.filter(([, payload]) => !!(payload as { leave_huddle?: unknown }).leave_huddle)
    .length;

/** Boot the provider on the fake node, connect (for a nodeUrl), and join the
 *  huddle; returns with the stubbed session created. */
const joinOnFakeNode = async () => {
  const fake = makeFakeNode();
  activeTransport = fake.transport;
  render(
    <DucktapeProvider transport={fake.transport}>
      <Probe />
    </DucktapeProvider>,
  );
  await waitFor(() => expect(capturedState?.status?.publicKey).toBe(SELF_HEX));
  act(() => capturedActions!.connectRemote("http://127.0.0.1:1"));
  await waitFor(() => expect(capturedState?.nodeUrl).toBeTruthy());
  await waitFor(() => expect(capturedState?.status?.publicKey).toBe(SELF_HEX));
  act(() => capturedActions!.joinHuddle("general"));
  await waitFor(() => expect(stubs.length).toBe(1));
  return fake;
};

beforeEach(() => {
  stubs.length = 0;
  capturedActions = null;
  capturedState = null;
});

// ── tests ───────────────────────────────────────────────

describe("huddle lifecycle", () => {
  it("ends the media and shows 'removed' when the finalized roster drops us", async () => {
    const fake = await joinOnFakeNode();
    act(() => stubs[0].onEvent({ kind: "status", status: "live" }));
    expect(capturedState?.voice.status).toBe("live");

    // The roster carries us once (seen) …
    fake.state.huddle = [member("me", SELF_NODE), member("bob", [1])];
    act(() => fake.finalize());
    await waitFor(() => expect(capturedState?.channels[0]?.huddle?.length).toBe(2));

    // … then drops us while the session is live.
    fake.state.huddle = [member("bob", [1])];
    act(() => fake.finalize());
    await waitFor(() => expect(capturedState?.voice.status).toBe("error"));
    expect(capturedState?.voice.error).toBe("removed");
    expect(stubs[0].stop).toHaveBeenCalled();
    // We are already out of the roster — removal must not submit a leave.
    expect(leaveSubmits(fake.transport)).toBe(0);
  });

  it("auto-reconnects once on an unexpected close, then fails honestly", async () => {
    const fake = await joinOnFakeNode();
    act(() => stubs[0].onEvent({ kind: "status", status: "live" }));

    // First unexpected close → a fresh session, "reconnecting", no leave.
    act(() => stubs[0].onEvent({ kind: "status", status: "closed" }));
    await waitFor(() => expect(stubs.length).toBe(2));
    expect(capturedState?.voice.status).toBe("reconnecting");
    expect(capturedState?.voice.channelId).toBe("general");
    expect(leaveSubmits(fake.transport)).toBe(0);

    // The re-established session's own 'connecting' must not demote the label.
    act(() => stubs[1].onEvent({ kind: "status", status: "connecting" }));
    expect(capturedState?.voice.status).toBe("reconnecting");

    act(() => stubs[1].onEvent({ kind: "status", status: "live" }));
    expect(capturedState?.voice.status).toBe("live");

    // A second close inside the damp window gives up: error card + leave.
    act(() => stubs[1].onEvent({ kind: "status", status: "closed" }));
    await waitFor(() => expect(capturedState?.voice.status).toBe("error"));
    expect(capturedState?.voice.error).toBe("connection");
    expect(capturedState?.voice.channelId).toBe("general"); // card stays visible
    expect(stubs.length).toBe(2); // no third session
    expect(leaveSubmits(fake.transport)).toBe(1);
  });

  it("surfaces a transient media note on a failed camera acquire", async () => {
    await joinOnFakeNode();
    act(() => stubs[0].onEvent({ kind: "status", status: "live" }));
    act(() => stubs[0].onEvent({ kind: "mediaNote", note: "camera-failed" }));
    expect(capturedState?.voice.mediaNote).toBe("camera-failed");
  });
});
