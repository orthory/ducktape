// Provider contract over a fake transport: hydration projects node state in,
// writes submit the exact wire msg, and block events trigger a re-query.

import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { BlockEvent, NodeTransport, SubmitReceipt } from "../../domain/transport";
import type { BlockKind, PageBlock } from "../../domain/pages-client";
import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";

// Switching nodes dials a new one via node-bootstrap. Mock only connectRemote
// so the switch lands on a benign, empty node (its status rejects → the "no
// running node" surface) with no real network — enough to prove the previous
// node's chain tip/blocks are dropped, not carried across.
vi.mock("../../domain/node-bootstrap", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../domain/node-bootstrap")>();
  const emptyNode: NodeTransport = {
    submit: vi.fn().mockResolvedValue({ height: 0, appHash: "00".repeat(32) }),
    query: vi.fn().mockResolvedValue({}),
    view: vi.fn().mockResolvedValue({ hits: [] }),
    putBlob: vi.fn().mockResolvedValue("00".repeat(32)),
    getBlob: vi.fn().mockResolvedValue(new Uint8Array()),
    filesStage: vi.fn(),
    filesCommit: vi.fn(),
    filesStat: vi.fn(),
    filesLs: vi.fn(),
    filesRead: vi.fn(),
    filesHistory: vi.fn(),
    status: vi.fn().mockRejectedValue(new Error("empty test node")),
    metrics: vi.fn().mockResolvedValue(""),
    blocks: vi.fn().mockResolvedValue([]),
    onBlock: vi.fn(() => () => {}),
  };
  return {
    ...actual,
    connectRemote: vi.fn((httpUrl: string) => ({
      transport: emptyNode,
      url: httpUrl,
      managed: false,
    })),
  };
});

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
      if (target === "identity") {
        return Promise.resolve({ users: [] });
      }
      if (target === "valset") {
        if (query === "residents") {
          return Promise.resolve({ residents: [[0xfe, 0xed]] });
        }
        return Promise.resolve({ validators: [[0xde, 0xad, 0xbe, 0xef]] });
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
    status: vi.fn().mockResolvedValue({
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 1,
      modules: [{ id: "chat", root: "cc".repeat(32) }],
    }),
    metrics: vi.fn().mockResolvedValue(""),
    onBlock: vi.fn((listener: (block: BlockEvent) => void) => {
      blockListeners.add(listener);
      return () => blockListeners.delete(listener);
    }),
    blocks: vi.fn().mockResolvedValue([]),
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
      <span data-testid="resident-keys">{state.residents.join(",")}</span>
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

  it("hydrates resident standing from valset", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);

    await waitFor(() => {
      expect(screen.getByTestId("resident-keys").textContent).toBe("feed");
    });
    expect(transport.query).toHaveBeenCalledWith("valset", "residents");
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

  // Regression: the live chain tip and the node's own durable block history
  // are per-node — both must be dropped on a node switch, or the new node's
  // explorer shows the previous node's rows (and its tip) as if current.
  it("drops the previous node's chain tip and blocks when switching nodes", async () => {
    const { transport, finalize } = makeFakeNode();
    // node 1 has durable block history AND a live block stream, so the switch
    // must zero BOTH (blocks 1→0, lastBlock 7→null).
    vi.mocked(transport.blocks).mockResolvedValue([
      {
        height: 7,
        hash: "aa".repeat(32),
        commitHash: "bb".repeat(32),
        ops: [
          {
            proposer: "cc".repeat(32),
            disposition: "applied",
            target: "chat",
            operations: [],
            payload: "{}",
            opHash: "dd".repeat(32),
          },
        ],
      },
    ]);
    renderConsole(transport);
    await waitFor(() => {
      expect(screen.getByTestId("connected").textContent).toBe("true");
      expect(capturedState!.blocks.length).toBe(1);
    });

    // node 1's ws stream lands a block → the ungated tip follows it.
    await act(async () => {
      finalize({ height: 7, appHash: "bb".repeat(32) });
    });
    expect(capturedState!.lastBlock).toBe(7);

    // switch to another node → the previous node's rows/tip must not linger.
    await act(async () => {
      capturedActions!.connectRemote("http://127.0.0.1:9999");
    });
    expect(capturedState!.lastBlock).toBeNull();
    expect(capturedState!.blocks.length).toBe(0);
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

// ── Pages: snapshots vs in-flight ops ───────────────────

describe("pages snapshot refresh vs in-flight ops", () => {
  // The Enter-key shape: op1 commits block A's text, op2 inserts block B —
  // both preconfirmed, both awaiting finalization. op1 settles a block
  // earlier, and its completion refresh re-queries a snapshot that reflects
  // op1 but NOT the still-pending op2. That stale snapshot must not replace
  // the pages slices: block B's row (and the focused textarea inside it)
  // would unmount, and the text projection would revert until op2 finalizes.
  it("holds the pages slices while a later page op is still in flight", async () => {
    const { transport } = makeFakeNode();

    // committed pages state — mutated ONLY when a held submit settles, the
    // same visibility contract as the real node (submit resolves at
    // finalization, queries reflect settled ops immediately after).
    const pageBlock = (patch: Partial<PageBlock> & { id: string }): PageBlock => ({
      parent: "p1",
      page: "p1",
      kind: "paragraph",
      text: "",
      checked: false,
      children: [],
      ...patch,
    });
    let committed = [
      pageBlock({ id: "p1", parent: null, kind: "page", text: "Plan", children: ["a"] }),
      pageBlock({ id: "a", text: "draft" }),
    ];
    let committedHeight = 1;
    const held: Array<{
      payload: {
        update_text?: { block_id: string; text: string };
        insert_block?: {
          parent: string;
          after: string | null;
          block: { id: string; kind: BlockKind; text: string };
        };
      };
      resolve: (receipt: SubmitReceipt) => void;
    }> = [];
    const settleNext = () => {
      const next = held.shift()!;
      const update = next.payload.update_text;
      const insert = next.payload.insert_block;
      if (update) {
        committed = committed.map((b) =>
          b.id === update.block_id ? { ...b, text: update.text } : b,
        );
      }
      if (insert) {
        committed = committed
          .map((b) =>
            b.id === insert.parent
              ? { ...b, children: [...b.children, insert.block.id] }
              : b,
          )
          .concat([
            pageBlock({
              id: insert.block.id,
              parent: insert.parent,
              kind: insert.block.kind,
              text: insert.block.text,
            }),
          ]);
      }
      committedHeight += 1;
      next.resolve({ height: committedHeight, appHash: "dd".repeat(32) });
    };

    const baseQuery = vi.mocked(transport.query).getMockImplementation()!;
    vi.mocked(transport.query).mockImplementation((target, query) => {
      if (target !== "pages") return baseQuery(target, query);
      if (query === "list_pages") {
        return Promise.resolve({
          page_list: [{ id: "p1", title: committed[0].text, parent: null }],
        });
      }
      if ((query as { get_page?: unknown }).get_page) {
        return Promise.resolve({ page: [...committed] });
      }
      return Promise.resolve({ comment_threads: [] });
    });
    const baseSubmit = vi.mocked(transport.submit).getMockImplementation()!;
    vi.mocked(transport.submit).mockImplementation((target, payload, origin) => {
      if (target !== "pages") return baseSubmit(target, payload, origin);
      return new Promise((resolve) =>
        held.push({ payload: payload as (typeof held)[number]["payload"], resolve }),
      );
    });
    // a FRESH ring array per pull (the shared mock reuses one instance), so a
    // state.blocks identity change marks "a refresh snapshot fully applied".
    vi.mocked(transport.blocks).mockImplementation(() => Promise.resolve([]));
    // an honest node's status height is never below a receipt it issued —
    // the read-your-writes floor refuses lagging snapshots, so the mock must
    // track the heights its own receipts hand out.
    const baseStatus = vi.mocked(transport.status).getMockImplementation()!;
    vi.mocked(transport.status).mockImplementation(() =>
      baseStatus().then((s) => ({ ...s, height: committedHeight })),
    );

    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );
    await act(async () => {
      capturedActions!.openPage("p1");
    });
    await waitFor(() =>
      expect(capturedState!.activePageBlocks.map((b) => b.id)).toEqual(["p1", "a"]),
    );

    // op1: commit A's text; op2: insert B after it — the Enter keystroke.
    await act(async () => {
      capturedActions!.updatePageBlockText({ blockId: "a", text: "hello world" });
    });
    await act(async () => {
      capturedActions!.insertPageBlock({
        blockId: "b",
        parent: "p1",
        after: "a",
        kind: "paragraph",
        text: "",
      });
    });
    expect(capturedState!.activePageBlocks.map((b) => b.id)).toEqual(["p1", "a", "b"]);

    // settle op1 alone; wait for its completion refresh to fully land (the
    // blocks ring is re-fetched as a fresh array every refresh, so identity
    // change marks the snapshot application).
    const blocksBefore = capturedState!.blocks;
    await act(async () => {
      settleNext();
    });
    await waitFor(() => {
      expect(capturedState!.ops["page-block/a"].phase).toBe("finalized");
      expect(capturedState!.blocks).not.toBe(blocksBefore);
    });

    // the stale snapshot (no block B) must NOT have clobbered op2's
    // projection: B stays, and A keeps its committed text.
    expect(capturedState!.activePageBlocks.map((b) => b.id)).toEqual(["p1", "a", "b"]);
    expect(capturedState!.activePageBlocks[1].text).toBe("hello world");

    // op2 settles: committed truth now carries both ops and replaces the
    // projections on its completion refresh.
    await act(async () => {
      settleNext();
    });
    await waitFor(() => {
      expect(capturedState!.ops["page-block/b"].phase).toBe("finalized");
      expect(capturedState!.activePageBlocks.map((b) => b.id)).toEqual(["p1", "a", "b"]);
      expect(capturedState!.activePageBlocks[1].text).toBe("hello world");
    });
  });
});
