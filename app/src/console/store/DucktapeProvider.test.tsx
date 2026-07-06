// Provider contract over a fake transport: hydration projects node state in,
// writes submit the exact wire msg, and block events trigger a re-query.

import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { BlockEvent, NodeTransport, SubmitReceipt } from "../../domain/transport";
import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";

// ── Fake node ───────────────────────────────────────────

const GENERAL_MESSAGE = {
  channel_id: "general",
  seq: 1,
  head: {
    message_id: "m1",
    author: { user: Array.from(new TextEncoder().encode("jess")) },
    blocks: [{ paragraph: [{ text: "hello", marks: [] }] }],
    created_at: 10,
    rev: 0,
    edited_at: null,
    base_rev: null,
    deleted: false,
    thread: null,
    reply_count: 0,
    last_reply_seq: null,
  },
  reactions: [],
  channel_head_seq: 1,
};

const wireChannel = (id: string, name: string, created_at: number) => ({
  id,
  name,
  created_at,
  head_seq: 0,
  post_policy: "open",
  hooks: [],
  pinned: [],
});

const makeFakeNode = () => {
  const blockListeners = new Set<(block: BlockEvent) => void>();
  // channel-aware mini-node: CreateChannel grows the list, MessagesLatest
  // answers per channel — a stale-pane regression needs the distinction
  const channels = [wireChannel("general", "General", 1)];
  const messagesByChannel: Record<string, (typeof GENERAL_MESSAGE)[]> = {
    general: [GENERAL_MESSAGE],
  };
  let forgeHead: string | null = null;
  const transport: NodeTransport = {
    submit: vi.fn((target: string, payload: unknown) => {
      const create = (payload as { create_channel?: { channel_id: string; name: string } })
        .create_channel;
      if (target === "chat" && create) {
        channels.push(wireChannel(create.channel_id, create.name, 2));
        messagesByChannel[create.channel_id] = [];
      }
      if (target === "forge" && (payload as { commit?: unknown }).commit) {
        forgeHead = "a".repeat(40);
      }
      return Promise.resolve({ height: 2, appHash: "bb".repeat(32) });
    }),
    query: vi.fn((target: string, query: unknown) => {
      if (target === "chat" && query === "channels") {
        return Promise.resolve({ channels: [...channels] });
      }
      const latest = (query as { messages_latest?: { channel_id: string } })
        .messages_latest;
      if (target === "chat" && latest) {
        return Promise.resolve({
          messages: messagesByChannel[latest.channel_id] ?? [],
        });
      }
      if (target === "chat") {
        return Promise.resolve({
          thread: { root: GENERAL_MESSAGE, replies: [] },
        });
      }
      if (target === "forge") {
        return Promise.resolve({ head: forgeHead });
      }
      if (target === "agent") {
        return Promise.resolve({ agents: [] });
      }
      if (target === "runs") {
        if (query === "watches") return Promise.resolve({ watches: [] });
        return Promise.resolve({ pending_runs: [] });
      }
      if (target === "profiles") {
        return Promise.resolve({ profiles: [] });
      }
      if (target === "valset") {
        if (query === "observers") {
          return Promise.resolve({ observers: [[0xfe, 0xed]] });
        }
        return Promise.resolve({ validators: [[0xde, 0xad, 0xbe, 0xef]] });
      }
      return Promise.resolve({});
    }),
    view: vi.fn().mockResolvedValue({ hits: [] }),
    putBlob: vi.fn().mockResolvedValue("ab".repeat(32)),
    getBlob: vi.fn().mockResolvedValue(new Uint8Array()),
    status: vi.fn().mockResolvedValue({
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 1,
      modules: [{ id: "chat", root: "cc".repeat(32) }],
    }),
    onBlock: vi.fn((listener: (block: BlockEvent) => void) => {
      blockListeners.add(listener);
      return () => blockListeners.delete(listener);
    }),
    telemetry: vi.fn().mockResolvedValue([]),
    blocks: vi.fn().mockResolvedValue([]),
    onTelemetry: vi.fn(() => () => {}),
  };
  const finalize = (block: BlockEvent) =>
    blockListeners.forEach((notify) => notify(block));
  return { transport, finalize };
};

let capturedActions: ConsoleActions | null = null;
let capturedState: ReturnType<typeof useDucktape>["state"] | null = null;

function Probe() {
  const { state, actions } = useDucktape();
  capturedActions = actions;
  capturedState = state;
  return (
    <div>
      <span data-testid="height">{state.status?.height ?? -1}</span>
      <span data-testid="channel">{state.activeChannel ?? "none"}</span>
      <span data-testid="messages">{state.messages.length}</span>
      <span data-testid="thread">{state.activeThread ? "open" : "closed"}</span>
      <span data-testid="forge">{state.forgeHead ?? "unborn"}</span>
      <span data-testid="members">{state.members.length}</span>
      <span data-testid="member-keys">{state.members.join(",")}</span>
      <span data-testid="observer-keys">{state.observers.join(",")}</span>
      <span data-testid="connected">{String(state.connected)}</span>
    </div>
  );
}

const renderConsole = (transport: NodeTransport) =>
  render(
    <DucktapeProvider transport={transport}>
      <Probe />
    </DucktapeProvider>,
  );

// ── Tests ───────────────────────────────────────────────

describe("DucktapeProvider", () => {
  it("hydrates status, adopts the first channel, and loads its messages", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);

    await waitFor(() => {
      expect(screen.getByTestId("height").textContent).toBe("1");
      expect(screen.getByTestId("channel").textContent).toBe("general");
      expect(screen.getByTestId("messages").textContent).toBe("1");
    });
  });

  it("hydrates validator members from valset", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);

    await waitFor(() => {
      expect(screen.getByTestId("members").textContent).toBe("1");
      expect(screen.getByTestId("member-keys").textContent).toBe("deadbeef");
    });
    expect(transport.query).toHaveBeenCalledWith("valset", "validators");
  });

  it("hydrates observer standing from valset", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);

    await waitFor(() => {
      expect(screen.getByTestId("observer-keys").textContent).toBe("feed");
    });
    expect(transport.query).toHaveBeenCalledWith("valset", "observers");
  });

  it("sendMessage posts a paragraph block with the author as submit origin", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      capturedActions!.sendMessage("hi node");
    });

    await waitFor(() => expect(transport.submit).toHaveBeenCalled());
    const [target, payload, origin] = vi.mocked(transport.submit).mock.calls[0];
    expect(target).toBe("chat");
    expect(origin).toBe("operator"); // authorship travels as origin, not payload
    const msg = payload as {
      post_message: {
        channel_id: string;
        message_id: string;
        blocks: unknown[];
        thread: number | null;
      };
    };
    expect(msg.post_message.channel_id).toBe("general");
    expect(msg.post_message.blocks).toEqual([
      { paragraph: [{ text: "hi node", marks: [] }] },
    ]);
    expect(msg.post_message.thread).toBeNull();
    expect(msg.post_message.message_id).toBeTruthy();
  });

  it("re-queries committed state when a block finalizes", async () => {
    const { transport, finalize } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    const statusCalls = vi.mocked(transport.status).mock.calls.length;
    await act(async () => {
      finalize({ height: 5, appHash: "dd".repeat(32) });
    });

    await waitFor(() =>
      expect(vi.mocked(transport.status).mock.calls.length).toBeGreaterThan(
        statusCalls,
      ),
    );
  });

  it("createChannel enters the new channel: its messages load, the thread closes", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    // open a thread first — creating a channel must close it
    await act(async () => {
      capturedActions!.openThread(1);
    });
    await waitFor(() =>
      expect(screen.getByTestId("thread").textContent).toBe("open"),
    );

    await act(async () => {
      capturedActions!.createChannel("Release Party", "open");
    });

    await waitFor(() => {
      expect(screen.getByTestId("channel").textContent).toBe("release-party");
      // the stale-pane bug left #general's message list (1) showing here
      expect(screen.getByTestId("messages").textContent).toBe("0");
      expect(screen.getByTestId("thread").textContent).toBe("closed");
    });
  });

  // Regression (found in headless QA): a node that goes silently unreachable —
  // it stops answering with no error and stops sending blocks — must flip the UI
  // to disconnected, and must auto-reconnect when it returns. The block stream
  // alone can't do this (silence is ambiguous: a healthy idle node sends none
  // either), so the liveness heartbeat polls status() and drives `connected`.
  it("detects a node that goes silently unreachable, then auto-reconnects", async () => {
    vi.useFakeTimers();
    try {
      const { transport } = makeFakeNode();
      let nodeUp = true;
      vi.mocked(transport.status).mockImplementation(() =>
        nodeUp
          ? Promise.resolve({
              version: "0.1.0",
              appHash: "aa".repeat(32),
              height: 1,
              modules: [],
            })
          : Promise.reject(new Error("connection refused")),
      );

      renderConsole(transport);
      // initial hydrate → connected
      await act(async () => {
        await vi.advanceTimersByTimeAsync(50);
      });
      expect(screen.getByTestId("connected").textContent).toBe("true");

      // node goes away: no error surfaces on the block stream, but the next
      // heartbeat's status() rejects → the UI must reflect disconnected.
      nodeUp = false;
      await act(async () => {
        await vi.advanceTimersByTimeAsync(3100);
      });
      expect(screen.getByTestId("connected").textContent).toBe("false");

      // node returns: the heartbeat's status() succeeds again → re-hydrate.
      nodeUp = true;
      await act(async () => {
        await vi.advanceTimersByTimeAsync(3100);
      });
      expect(screen.getByTestId("connected").textContent).toBe("true");
    } finally {
      vi.useRealTimers();
    }
  });

  it("commitForge submits a Commit msg and hydrates the new HEAD", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("forge").textContent).toBe("unborn"),
    );

    await act(async () => {
      capturedActions!.commitForge({
        path: "README.md",
        content: "hello forge",
        message: "init",
      });
    });

    await waitFor(() =>
      expect(screen.getByTestId("forge").textContent).toBe("a".repeat(40)),
    );
    const forgeCall = vi
      .mocked(transport.submit)
      .mock.calls.find((call) => call[0] === "forge");
    expect(forgeCall).toBeTruthy();
    expect(forgeCall![1]).toEqual({
      commit: { path: "README.md", content: "hello forge", message: "init" },
    });
  });
});

// ── Preconfirmed render + finalization ledger ───────────

describe("submitTracked lifecycle", () => {
  it("renders the op preconfirmed, then finalizes the ledger from the receipt", async () => {
    const { transport } = makeFakeNode();
    let settle!: (receipt: SubmitReceipt) => void;
    vi.mocked(transport.submit).mockImplementation(
      () => new Promise((resolve) => (settle = resolve)),
    );
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      capturedActions!.sendMessage("preconfirm me");
    });

    // BEFORE the node answers: the message already renders, and its ledger
    // record is pending under the minted message-id key.
    expect(screen.getByTestId("messages").textContent).toBe("2");
    const pendingKeys = Object.keys(capturedState!.ops);
    expect(pendingKeys).toHaveLength(1);
    expect(pendingKeys[0]).toMatch(/^chat\/general\/id\//);
    expect(capturedState!.ops[pendingKeys[0]].phase).toBe("pending");

    await act(async () => {
      settle({ height: 9, appHash: "dd".repeat(32), opHash: "ee".repeat(32) });
    });

    await waitFor(() => {
      const record = capturedState!.ops[pendingKeys[0]];
      expect(record.phase).toBe("finalized");
      expect(record.height).toBe(9);
      expect(record.opHash).toBe("ee".repeat(32));
    });
  });

  it("rolls the preconfirmed render back and records the rejection on failure", async () => {
    const { transport } = makeFakeNode();
    let reject!: (err: Error) => void;
    vi.mocked(transport.submit).mockImplementation(
      () => new Promise((_resolve, rej) => (reject = rej)),
    );
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      capturedActions!.sendMessage("doomed");
    });
    expect(screen.getByTestId("messages").textContent).toBe("2");
    const key = Object.keys(capturedState!.ops)[0];

    await act(async () => {
      reject(new Error("chat: channel is members-only"));
    });

    // the rollback refresh restores committed truth; the record keeps the why.
    await waitFor(() => {
      expect(screen.getByTestId("messages").textContent).toBe("1");
      expect(capturedState!.ops[key].phase).toBe("failed");
      expect(capturedState!.ops[key].error).toContain("members-only");
    });
  });
});
