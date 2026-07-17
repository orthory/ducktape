// The focused history window (loadMessageWindow): a jump-to-message older than
// the channel's loaded tail replaces `messages` with a window centered on that
// seq, and re-arms the focus so ChatView can scroll+flash the row.
//
// The window is also the REQUEST TOKEN. Both exits from a window — posting
// (postToChannel) and re-entering the channel / "Jump to latest" (enterChannel)
// — clear `chatWindow` synchronously while the window's own round trip can
// still be in flight, so every applier on that read is gated on the token still
// matching. These tests script that race with a deferred query instead of
// hoping for the timing.
//
// Fake transport straight into the provider (same shape as
// DucktapeProvider.test.tsx), because the race is a store fact, not a view one.

import { act, render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { blocksText } from "../../domain/chat-client";
import type { MessageView } from "../../domain/chat-client";
import type { NodeTransport } from "../../domain/transport";
import { makeTransportStub } from "../../test/transport-stub";
import { DucktapeProvider } from "./DucktapeProvider";
import type { ConsoleActions } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";

// Desktop-only effects, mocked exactly as DucktapeProvider.test.tsx does so the
// provider's mount effects run in jsdom.
vi.mock("../../domain/notify-client", () => ({
  configure: vi.fn(() => Promise.resolve()),
  markSeen: vi.fn(() => Promise.resolve()),
  onUnread: vi.fn(() => Promise.resolve(() => {})),
}));
const message = (seq: number, text: string): MessageView => ({
  channel_id: "general",
  seq,
  head: {
    message_id: `m-${seq}`,
    author: { user: Array.from(new TextEncoder().encode("operator")) },
    blocks: [{ paragraph: [{ text, marks: [] }] }],
    created_at: 1_000 + seq,
    rev: 1,
    edited_at: null,
    base_rev: null,
    deleted: false,
    thread: null,
    reply_count: 0,
    last_reply_seq: null,
  },
  reactions: [],
  channel_head_seq: 900,
});

/** The tail slice `messages_latest` answers with, and the far-older window
 *  `messages_around` answers with — disjoint, so which one is on screen is
 *  unambiguous. */
const TAIL = [message(900, "tail")];
const WINDOW = [message(12, "history")];

const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
};

interface FakeNode {
  transport: NodeTransport;
  /** Hand back the pending messages_around round trip (one per call). */
  window: () => { resolve: (value: unknown) => void; reject: (reason: unknown) => void };
  aroundCalls: () => number;
  /** Let a post whose receipt was held (`holdPosts`) land. */
  settlePost: () => void;
}

/** A channel-aware mini-node: the tail grows when a post is submitted (a real
 *  node's post takes the HEAD sequence), and `messages_around` is DEFERRED —
 *  the test decides when, and in which order, that response lands.
 *
 *  `holdPosts` also defers the post's RECEIPT, which holds back the refresh
 *  that submitTracked fires on it — the only way to keep an earlier read the
 *  live one (a later hydrate bumps the generation and invalidates it anyway). */
const makeFakeNode = ({ holdPosts = false }: { holdPosts?: boolean } = {}): FakeNode => {
  const tail = [...TAIL];
  const pending: Array<ReturnType<typeof deferred<unknown>>> = [];
  const heldPosts: Array<ReturnType<typeof deferred<unknown>>> = [];
  let aroundCalls = 0;
  // the tip advances with every committed write — a status BELOW a receipt this
  // console holds would be skipped by the read-your-writes floor, and a refresh
  // that never applies anything proves nothing.
  let height = 1;
  const transport = makeTransportStub({
    status: vi.fn(() =>
      Promise.resolve({
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height,
        modules: [{ id: "chat", root: "cc".repeat(32) }],
      }),
    ),
    submit: vi.fn((target: string, payload: unknown) => {
      const post = (payload as { post_message?: { blocks: unknown[] } }).post_message;
      height += 1;
      const receipt = { height, appHash: "bb".repeat(32) };
      if (target === "chat" && post) {
        tail.push(message(901, blocksText(post.blocks as MessageView["head"]["blocks"])));
        if (holdPosts) {
          const held = deferred<unknown>();
          heldPosts.push(held);
          return held.promise.then(() => receipt);
        }
      }
      return Promise.resolve(receipt);
    }),
    query: vi.fn((target: string, query: unknown) => {
      if (target === "chat" && query === "channels") {
        return Promise.resolve({
          channels: [
            {
              id: "general",
              name: "general",
              created_at: 1,
              head_seq: 900,
              post_policy: "open",
              hooks: [],
              pinned: [],
            },
          ],
        });
      }
      if (target === "chat" && (query as { messages_latest?: unknown }).messages_latest) {
        return Promise.resolve({ messages: [...tail] });
      }
      if (target === "chat" && (query as { messages_around?: unknown }).messages_around) {
        aroundCalls += 1;
        const round = deferred<unknown>();
        pending.push(round);
        return round.promise;
      }
      if (target === "valset") {
        return Promise.resolve(
          query === "residents" ? { residents: [] } : { validators: [] },
        );
      }
      if (target === "runs") {
        return Promise.resolve(query === "watches" ? { watches: [] } : { pending_runs: [] });
      }
      if (target === "forge") return Promise.resolve({ head: null });
      if (target === "agent") return Promise.resolve({ agents: [] });
      if (target === "identity") return Promise.resolve({ accounts: [] });
      return Promise.resolve({});
    }),
  });
  return {
    transport,
    window: () => {
      const round = pending.shift();
      if (!round) throw new Error("no messages_around request in flight");
      return round;
    },
    aroundCalls: () => aroundCalls,
    settlePost: () => heldPosts.shift()?.resolve(null),
  };
};

let capturedActions: ConsoleActions | null = null;
let capturedState: ReturnType<typeof useDucktape>["state"] | null = null;

function Probe() {
  const { state, actions } = useDucktape();
  capturedState = state;
  capturedActions = actions;
  return null;
}

const texts = () => capturedState!.messages.map((m) => blocksText(m.head.blocks));

/** Boot the console onto the fake node, parked on the channel's tail. */
const bootOnTail = async (transport: NodeTransport) => {
  render(
    <DucktapeProvider transport={transport}>
      <Probe />
    </DucktapeProvider>,
  );
  await waitFor(() => {
    expect(capturedState!.activeChannel).toBe("general");
    expect(texts()).toEqual(["tail"]);
  });
};

/** Ask for the window and let the query go out (chat-client issues it on a
 *  microtask) — WITHOUT resolving it: the round trip stays in flight, which is
 *  the state every race below needs. */
const startWindow = async (seq: number) => {
  await act(async () => {
    capturedActions!.loadMessageWindow("general", seq);
  });
};

describe("loadMessageWindow", () => {
  it("swaps in the window around the seq and re-arms the focus", async () => {
    const node = makeFakeNode();
    await bootOnTail(node.transport);

    await startWindow(12);
    // marked BEFORE the round trip — it is the token, and ChatView's record
    // that this seq was already asked for.
    expect(capturedState!.chatWindow).toEqual({ channelId: "general", seq: 12 });

    await act(async () => {
      node.window().resolve({ messages: WINDOW });
    });

    await waitFor(() => expect(texts()).toEqual(["history"]));
    // re-armed: the row exists now, so ChatView's next pass scrolls+flashes it.
    expect(capturedState!.chatFocusSeq).toBe(12);
    expect(capturedState!.chatWindow).toEqual({ channelId: "general", seq: 12 });
  });

  it("does not erase a message posted while the window request was in flight", async () => {
    // THE RACE. Posting is an exit from the window: it clears `chatWindow` and
    // paints the optimistic message. The window response is still in flight —
    // and it must NOT land, or the just-posted message vanishes from a reader
    // who is now looking at the tail with no history bar left to escape by.
    const node = makeFakeNode();
    await bootOnTail(node.transport);

    await startWindow(12);
    expect(capturedState!.chatWindow).not.toBeNull();

    await act(async () => {
      capturedActions!.sendMessage("posted");
    });
    await waitFor(() => expect(texts()).toContain("posted"));
    expect(capturedState!.chatWindow).toBeNull(); // the post left the window

    // ...and only NOW does the superseded window response resolve.
    await act(async () => {
      node.window().resolve({ messages: WINDOW });
    });

    expect(texts()).toContain("posted");
    expect(texts()).not.toContain("history");
    expect(capturedState!.chatWindow).toBeNull();
    expect(capturedState!.chatFocusSeq).toBeNull(); // no focus re-armed either
  });

  it("does not erase a post with the WINDOW a hydrate was already fetching", async () => {
    // The same race one layer up. While a window is up, a refresh re-pulls THAT
    // window (fetchChatSlices takes state.chatWindow). Post while that read is
    // in flight and the window it returns must not be applied as `messages` —
    // it would erase the optimistic post. The provider re-checks the window the
    // read was taken FOR.
    const node = makeFakeNode({ holdPosts: true });
    await bootOnTail(node.transport);

    await startWindow(12);
    await act(async () => {
      node.window().resolve({ messages: WINDOW });
    });
    await waitFor(() => expect(texts()).toEqual(["history"]));

    // any write refreshes (here: reacting to a message in the window) — and the
    // window is still up, so the refresh re-reads the WINDOW.
    await act(async () => {
      capturedActions!.toggleReaction(12, "👍");
    });

    // the reader posts before that read comes back. The post's OWN refresh is
    // held (its submit is deferred) so nothing else supersedes the read.
    await act(async () => {
      capturedActions!.sendMessage("posted");
    });
    await waitFor(() => expect(texts()).toContain("posted"));
    expect(capturedState!.chatWindow).toBeNull();

    await act(async () => {
      node.window().resolve({ messages: WINDOW });
    });
    expect(texts()).toContain("posted");

    node.settlePost();
  });

  it("keeps a 'Jump to latest' taken mid-flight — the late window does not re-install", async () => {
    const node = makeFakeNode();
    await bootOnTail(node.transport);

    await startWindow(12);
    // "Jump to latest" (and a rail click on the channel) is selectChannel.
    await act(async () => {
      capturedActions!.selectChannel("general");
    });
    expect(capturedState!.chatWindow).toBeNull();

    await act(async () => {
      node.window().resolve({ messages: WINDOW });
    });

    expect(texts()).toEqual(["tail"]);
    expect(capturedState!.chatWindow).toBeNull();
  });

  it("degrades to the tail with one error when the node cannot answer the query", async () => {
    // A node too old to know the messages_around variant rejects it outright.
    const node = makeFakeNode();
    await bootOnTail(node.transport);

    await startWindow(12);
    await act(async () => {
      node.window().reject(new Error("unknown query variant"));
    });

    // stays on the tail enterChannel already loaded, and SAYS so.
    expect(texts()).toEqual(["tail"]);
    expect(capturedState!.chatWindow).toBeNull();
    expect(capturedState!.error).toMatch(/too old to page it in/i);
    // asked once — the store does not retry the query it just failed.
    expect(node.aroundCalls()).toBe(1);
  });
});
