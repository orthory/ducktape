// Provider contract over a fake transport: hydration projects node state in,
// writes submit the exact wire msg, and block events trigger a re-query.

import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentRecord } from "../../domain/agent-client";
import type { AccountView } from "../../domain/identity-client";
import { moduleTopic } from "../../domain/stream";
import type { EventFrame, HeartbeatFrame } from "../../domain/stream";
import type { NodeTransport, SubmitReceipt, StreamSignal, TopicHandlers } from "../../domain/transport";
import type { BlockKind, PageBlock } from "../../domain/pages-client";
import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";
import { makeTransportStub } from "../../test/transport-stub";

// The desktop-only effects (notify config push, navigate deep-link) talk to
// the Rust side through notify-client and the Tauri event plane — both mocked
// so desktop-path tests can run in jsdom and observe/emit directly.
const notifyMocks = vi.hoisted(() => ({
  configure: vi.fn(() => Promise.resolve()),
  markSeen: vi.fn(() => Promise.resolve()),
  onUnread: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("../../domain/notify-client", () => notifyMocks);

const tauriEvent = vi.hoisted(() => {
  const handlers = new Map<string, Set<(event: { payload: unknown }) => void>>();
  return {
    handlers,
    /** Fire a Tauri event into every registered listener (the test's Rust). */
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
vi.mock("@tauri-apps/api/event", () => ({
  listen: tauriEvent.listen,
  emit: tauriEvent.emit,
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
// Materialize the mocked module up front: the provider fires several
// CONCURRENT dynamic imports of the event module on mount, and vitest's lazy
// mock factory races them — the loser would fall through to the real module.
import "@tauri-apps/api/event";
import "@tauri-apps/api/core";

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
    subscribe: vi.fn(() => () => {}),
    onStream: vi.fn(() => () => {}),
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

const JESS_USER_KEY = Array.from({ length: 32 }, (_, index) => index);

const JESS_USER: AccountView = {
  account_id: JESS_USER_KEY,
  display_name: "Jess K",
  nonce: 0,
  member_keys: [],
  nodes: [[0xaa]],
  updated_at: 1,
};

const QUACKBOT: AgentRecord = {
  agent_id: "quackbot",
  owner: { external: [1] },
  display_name: "Quackbot",
  capability: "echo",
  prompt_hash: Array(32).fill(7),
  allowed_actions: ["chat.post"],
  status: "active",
  created_at: 1,
  updated_at: 1,
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

const makeFakeNode = ({
  agents = [],
  users = [],
  publicKey,
}: { agents?: AgentRecord[]; users?: AccountView[]; publicKey?: string } = {}) => {
  const topicHandlers = new Map<string, Set<TopicHandlers>>();
  const streamListeners = new Set<(signal: StreamSignal) => void>();
  // channel-aware mini-node: CreateChannel grows the list, MessagesLatest
  // answers per channel — a stale-pane regression needs the distinction
  const channels = [wireChannel("general", "General", 1)];
  const messagesByChannel: Record<string, (typeof GENERAL_MESSAGE)[]> = {
    general: [GENERAL_MESSAGE],
  };
  let forgeHead: string | null = null;
  const nodeStatus = {
    version: "0.1.0",
    appHash: "aa".repeat(32),
    height: 1,
    // identity rides along so an emitOps("identity", …) can roll its root and
    // scope a hydrate to the people slices (the notify fingerprint-dedupe test).
    modules: [
      { id: "chat", root: "cc".repeat(32) },
      { id: "agent", root: "ee".repeat(32) },
      { id: "identity", root: "11".repeat(32) },
    ],
  };
  const transport: NodeTransport = makeTransportStub({
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
        return Promise.resolve({ agents });
      }
      if (target === "runs") {
        if (query === "watches") return Promise.resolve({ watches: [] });
        return Promise.resolve({ pending_runs: [] });
      }
      if (target === "identity") {
        return Promise.resolve({ accounts: users });
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
    // dynamic: emitOps advances the height and rolls the emitted module's
    // root, so refreshScoped's root diff sees exactly what "the block"
    // touched. deep-copied per call — the provider stores the previous
    // status and diffs against the next one; aliasing would blank the diff.
    status: vi.fn(() =>
      Promise.resolve({
        version: nodeStatus.version,
        appHash: nodeStatus.appHash,
        height: nodeStatus.height,
        modules: nodeStatus.modules.map((m) => ({ ...m })),
        ...(publicKey ? { publicKey } : {}),
      }),
    ),
    metrics: vi.fn().mockResolvedValue(""),
    blocks: vi.fn().mockResolvedValue([]),
    subscribe: vi.fn((topics: string[], handlers: TopicHandlers) => {
      for (const topic of topics) {
        let handlersForTopic = topicHandlers.get(topic);
        if (!handlersForTopic) {
          handlersForTopic = new Set();
          topicHandlers.set(topic, handlersForTopic);
        }
        handlersForTopic.add(handlers);
      }
      return () => {
        for (const topic of topics) {
          const handlersForTopic = topicHandlers.get(topic);
          if (!handlersForTopic) continue;
          handlersForTopic.delete(handlers);
          if (handlersForTopic.size === 0) topicHandlers.delete(topic);
        }
      };
    }),
    onStream: vi.fn((listener: (signal: StreamSignal) => void) => {
      streamListeners.add(listener);
      return () => streamListeners.delete(listener);
    }),
  });
  const emitOps = (
    module: string,
    rows: Array<Partial<EventFrame["op"]> & { height: number }>,
  ) => {
    const topic = moduleTopic(module);
    rows.forEach((row, index) => {
      const frame: EventFrame = {
        type: "event",
        topic,
        cursor: `op/${row.height.toString(16).padStart(16, "0")}/${String(index).padStart(4, "0")}`,
        op: {
          seq: index,
          time: row.time ?? row.height,
          origin: row.origin ?? { kind: "external", id: "tester" },
          ...row,
        },
      };
      topicHandlers.get(topic)?.forEach((handlers) => handlers.onEvent?.(frame));
    });
    // "the block" this batch represents: advance the tip and roll the folded
    // module's root so the next scoped hydrate diffs exactly this module.
    const tip = Math.max(...rows.map((row) => row.height));
    if (tip > nodeStatus.height) nodeStatus.height = tip;
    const folded = nodeStatus.modules.find((m) => m.id === module);
    if (folded) {
      folded.root = tip.toString(16).padStart(2, "0").repeat(32).slice(0, 64);
    }
  };
  const emitHeartbeat = (height: number, appHash = "dd".repeat(32)) => {
    const frame: HeartbeatFrame = {
      type: "heartbeat",
      height,
      appHash,
      timeMs: Date.now(),
      intervalMs: 3_000,
    };
    streamListeners.forEach((notify) => notify({ kind: "heartbeat", frame }));
  };
  const emitDown = (reason = "connection refused") =>
    streamListeners.forEach((notify) => notify({ kind: "down", reason }));
  const emitUp = () =>
    streamListeners.forEach((notify) => notify({ kind: "up" }));
  return { transport, emitOps, emitHeartbeat, emitDown, emitUp };
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

  it("sendMessage resolves a workspace user mention without creating an agent watch", async () => {
    const { transport } = makeFakeNode({ users: [JESS_USER] });
    renderConsole(transport);
    await waitFor(() => {
      expect(screen.getByTestId("channel").textContent).toBe("general");
      expect(Object.keys(capturedState!.nodeUsers)).toHaveLength(1);
    });

    await act(async () => {
      capturedActions!.sendMessage("hi @jess-k");
    });

    await waitFor(() => expect(transport.submit).toHaveBeenCalledTimes(1));
    expect(vi.mocked(transport.submit).mock.calls[0]).toEqual([
      "chat",
      {
        post_message: {
          channel_id: "general",
          message_id: expect.any(String),
          blocks: [
            {
              paragraph: [
                { text: "hi ", marks: [] },
                {
                  text: "@jess-k",
                  marks: [{ mention: { user: JESS_USER_KEY } }],
                },
              ],
            },
          ],
          thread: null,
          as_agent: null,
        },
      },
      "operator",
    ]);
    expect(vi.mocked(transport.submit).mock.calls.some(([target]) => target === "runs"))
      .toBe(false);
  });

  it("sendMessage preserves agent mention resolution and creates the watch before posting", async () => {
    const { transport } = makeFakeNode({ agents: [QUACKBOT] });
    renderConsole(transport);
    await waitFor(() => {
      expect(screen.getByTestId("channel").textContent).toBe("general");
      expect(capturedState!.agents).toEqual([QUACKBOT]);
    });

    await act(async () => {
      capturedActions!.sendMessage("hi @quackbot");
    });

    await waitFor(() => expect(transport.submit).toHaveBeenCalledTimes(2));
    expect(vi.mocked(transport.submit).mock.calls[0]).toEqual([
      "runs",
      {
        watch_channel: {
          channel_id: "general",
          policy: "mention",
        },
      },
      "operator",
    ]);
    expect(vi.mocked(transport.submit).mock.calls[1]).toEqual([
      "chat",
      {
        post_message: {
          channel_id: "general",
          message_id: expect.any(String),
          blocks: [
            {
              paragraph: [
                { text: "hi ", marks: [] },
                {
                  text: "@quackbot",
                  marks: [
                    {
                      mention: {
                        agent: { module: "runs", agent_id: "quackbot" },
                      },
                    },
                  ],
                },
              ],
            },
          ],
          thread: null,
          as_agent: null,
        },
      },
      "operator",
    ]);
  });

  it("replyInThread resolves a workspace user mention in the submitted blocks", async () => {
    const { transport } = makeFakeNode({ users: [JESS_USER] });
    renderConsole(transport);
    await waitFor(() => {
      expect(screen.getByTestId("channel").textContent).toBe("general");
      expect(Object.keys(capturedState!.nodeUsers)).toHaveLength(1);
    });
    await act(async () => {
      capturedActions!.openThread(1);
    });
    await waitFor(() => expect(screen.getByTestId("thread").textContent).toBe("open"));

    await act(async () => {
      capturedActions!.replyInThread("reply @jess-k");
    });

    await waitFor(() => expect(transport.submit).toHaveBeenCalledTimes(1));
    expect(vi.mocked(transport.submit).mock.calls[0][1]).toMatchObject({
      post_message: {
        blocks: [
          {
            paragraph: [
              { text: "reply ", marks: [] },
              {
                text: "@jess-k",
                marks: [{ mention: { user: JESS_USER_KEY } }],
              },
            ],
          },
        ],
        thread: 1,
      },
    });
  });

  it("editMessage resolves a workspace user mention in the submitted blocks", async () => {
    const { transport } = makeFakeNode({ users: [JESS_USER] });
    renderConsole(transport);
    await waitFor(() => {
      expect(screen.getByTestId("channel").textContent).toBe("general");
      expect(Object.keys(capturedState!.nodeUsers)).toHaveLength(1);
    });

    await act(async () => {
      capturedActions!.editMessage(1, "edited @jess-k");
    });

    await waitFor(() => expect(transport.submit).toHaveBeenCalledTimes(1));
    expect(vi.mocked(transport.submit).mock.calls[0]).toEqual([
      "chat",
      {
        edit_message: {
          channel_id: "general",
          seq: 1,
          blocks: [
            {
              paragraph: [
                { text: "edited ", marks: [] },
                {
                  text: "@jess-k",
                  marks: [{ mention: { user: JESS_USER_KEY } }],
                },
              ],
            },
          ],
          base_rev: 0,
        },
      },
      "operator",
    ]);
  });

  it("a chat event triggers a scoped hydrate: chat refetches, agents don't", async () => {
    const { transport, emitOps } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    const chatCalls = vi
      .mocked(transport.query)
      .mock.calls.filter((call) => call[0] === "chat").length;
    const agentCalls = vi
      .mocked(transport.query)
      .mock.calls.filter((call) => call[0] === "agent").length;
    await act(async () => {
      emitOps("chat", [{ height: 5 }]);
      await new Promise((resolve) => setTimeout(resolve, 150));
    });

    // the event drives refreshScoped: one status read roots the diff, the
    // chat slice group re-queries, and untouched groups (agents) stay quiet.
    await waitFor(() =>
      expect(
        vi.mocked(transport.query).mock.calls.filter((call) => call[0] === "chat")
          .length,
      ).toBeGreaterThan(chatCalls),
    );
    expect(
      vi.mocked(transport.query).mock.calls.filter((call) => call[0] === "agent")
        .length,
    ).toBe(agentCalls);
    expect(capturedState!.lastBlock).toBe(5);
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
  it("marks down on stream watchdog death, then recovers on the up edge", async () => {
    const { transport, emitDown, emitUp } = makeFakeNode();

    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("connected").textContent).toBe("true"),
    );

    await act(async () => {
      emitDown("stream heartbeat timed out");
    });
    expect(screen.getByTestId("connected").textContent).toBe("false");

    await act(async () => {
      emitUp();
    });
    await waitFor(() =>
      expect(screen.getByTestId("connected").textContent).toBe("true"),
    );
  });

  it("rechecks the workspace node identity on the up edge", async () => {
    const { transport, emitDown, emitUp } = makeFakeNode();
    vi.mocked(transport.status).mockResolvedValue({
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 1,
      modules: [{ id: "chat", root: "cc".repeat(32) }],
      publicKey: "badc0de",
    });

    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("connected").textContent).toBe("true"),
    );
    capturedState!.workspace = {
      id: "w1",
      name: "Workspace",
      chainId: "chain",
      pubkey: "expected",
      founder: true,
      member: true,
      ports: { listen: 1, http: 2, rpc: 3 },
    };

    await act(async () => {
      emitDown("stream heartbeat timed out");
    });
    await waitFor(() =>
      expect(screen.getByTestId("connected").textContent).toBe("false"),
    );
    await act(async () => {
      emitUp();
    });
    await waitFor(() =>
      expect(capturedState!.connectionDown?.impostor).toBe(true),
    );
    expect(screen.getByTestId("connected").textContent).toBe("false");
  });

  // Regression: the live chain tip and the node's own durable block history
  // are per-node — both must be dropped on a node switch, or the new node's
  // explorer shows the previous node's rows (and its tip) as if current.
  it("drops the previous node's chain tip and blocks when switching nodes", async () => {
    const { transport, emitOps } = makeFakeNode();
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
      emitOps("chat", [{ height: 7 }]);
      await new Promise((resolve) => setTimeout(resolve, 150));
    });
    expect(capturedState!.lastBlock).toBe(7);

    // switch to another node → the previous node's rows/tip must not linger.
    await act(async () => {
      capturedActions!.connectRemote("http://127.0.0.1:9999");
    });
    expect(capturedState!.lastBlock).toBeNull();
    expect(capturedState!.blocks.length).toBe(0);
  });

  it("ignores an old node hydrate that settles after switching nodes", async () => {
    const { transport } = makeFakeNode({ agents: [QUACKBOT] });
    let releaseAgents!: () => void;
    const agentsGate = new Promise<void>((resolve) => {
      releaseAgents = resolve;
    });
    const baseQuery = vi.mocked(transport.query).getMockImplementation()!;
    vi.mocked(transport.query).mockImplementation((target, query) => {
      if (target === "agent") {
        return agentsGate.then(() => ({ agents: [QUACKBOT] }));
      }
      if (target === "pages" && query === "list_pages") {
        return Promise.resolve({
          pages: [{ id: "old-page", title: "Old workspace", parent: null }],
        });
      }
      return baseQuery(target, query);
    });

    renderConsole(transport);
    await waitFor(() => expect(transport.status).toHaveBeenCalled());

    await act(async () => {
      capturedActions!.connectRemote("http://127.0.0.1:9999");
    });
    expect(capturedState!.agents).toEqual([]);
    expect(capturedState!.pages).toEqual([]);

    // The old node's full hydrate finishes after the reset. It must be a
    // no-op: neither its agent roster nor its Pages enumeration belongs to
    // the newly selected node.
    await act(async () => {
      releaseAgents();
      await agentsGate;
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(capturedState!.agents).toEqual([]);
    expect(capturedState!.pages).toEqual([]);
  });

  it("ignores an old page load that settles after switching nodes", async () => {
    const { transport } = makeFakeNode();
    let releasePage!: () => void;
    const pageGate = new Promise<void>((resolve) => {
      releasePage = resolve;
    });
    const oldRoot: PageBlock = {
      id: "old-page",
      parent: null,
      page: "old-page",
      kind: "page",
      text: "Old workspace",
      checked: false,
      children: [],
    };
    const baseQuery = vi.mocked(transport.query).getMockImplementation()!;
    vi.mocked(transport.query).mockImplementation((target, query) => {
      if (target === "pages" && (query as { get_page?: unknown }).get_page) {
        return pageGate.then(() => ({ page: [oldRoot] }));
      }
      return baseQuery(target, query);
    });

    renderConsole(transport);
    await waitFor(() => expect(screen.getByTestId("connected").textContent).toBe("true"));
    act(() => capturedActions!.openPage("old-page"));
    expect(capturedState!.activePage).toBe("old-page");

    await act(async () => {
      capturedActions!.connectRemote("http://127.0.0.1:9999");
    });
    expect(capturedState!.activePage).toBeNull();

    await act(async () => {
      releasePage();
      await pageGate;
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(capturedState!.activePageBlocks).toEqual([]);
    expect(capturedState!.pageThreads).toEqual([]);
  });

  it("does not run an old write's UI continuation in the new workspace", async () => {
    const { transport } = makeFakeNode();
    let settleCreate!: (receipt: SubmitReceipt) => void;
    const baseSubmit = vi.mocked(transport.submit).getMockImplementation()!;
    vi.mocked(transport.submit).mockImplementation((target, payload, origin) => {
      if (target === "chat" && (payload as { create_channel?: unknown }).create_channel) {
        return new Promise((resolve) => {
          settleCreate = resolve;
        });
      }
      return baseSubmit(target, payload, origin);
    });

    renderConsole(transport);
    await waitFor(() => expect(screen.getByTestId("connected").textContent).toBe("true"));
    act(() => capturedActions!.createChannel("Old Room", "open"));
    await waitFor(() => expect(transport.submit).toHaveBeenCalled());

    await act(async () => {
      capturedActions!.connectRemote("http://127.0.0.1:9999");
    });
    expect(capturedState!.activeChannel).toBeNull();

    // The old create receipt used to continue into enterChannel("old-room")
    // after the reset, reopening old coordinates against the new node.
    await act(async () => {
      settleCreate({ height: 2, appHash: "bb".repeat(32) });
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(capturedState!.activeChannel).toBeNull();
    expect(capturedState!.channels).toEqual([]);
    expect(capturedState!.ops).toEqual({});
  });

  it("clears every node-scoped projection when switching nodes", async () => {
    const { transport } = makeFakeNode({ agents: [QUACKBOT] });
    renderConsole(transport);
    await waitFor(() => expect(capturedState!.agents).toEqual([QUACKBOT]));

    // Seed query-driven and secondary slices that the old hand-written reset
    // lists omitted. They are all owned by the current node/workspace.
    Object.assign(capturedState!, {
      tagFilter: { tag: "old", channelId: "general" },
      pageThreads: [{ target: "old-page", threads: [] }],
      capabilities: ["old-capability"],
      capabilitiesByNode: new Map([["old-node", ["old-capability"]]]),
      runLease: new Map([["old-run", {}]]),
      forgeRepo: "old-repo",
      search: { query: "old", chat: [], docs: [] },
      openTabs: ["old-page"],
    });

    await act(async () => {
      capturedActions!.connectRemote("http://127.0.0.1:9999");
    });

    expect(capturedState!.tagFilter).toBeNull();
    expect(capturedState!.pageThreads).toEqual([]);
    expect(capturedState!.capabilities).toEqual([]);
    expect(capturedState!.capabilitiesByNode.size).toBe(0);
    expect(capturedState!.runLease.size).toBe(0);
    expect(capturedState!.forgeRepo).toBeNull();
    expect(capturedState!.search).toBeNull();
    expect(capturedState!.openTabs).toEqual([]);
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

// ── Desktop notifier: config push + deep-link navigation ─

// The account this desktop "is": two nodes bound to one identity account.
// status().publicKey (deliberately uppercase — the push must lowercase) names
// node aa; the payload's selfNodeKeysHex must carry BOTH of the account's
// nodes.
const SELF_ACCOUNT: AccountView = {
  account_id: JESS_USER_KEY,
  display_name: "Jess K",
  nonce: 0,
  member_keys: [],
  nodes: [[0xaa], [0xbb]],
  updated_at: 1,
};
const SELF_ACCOUNT_HEX = JESS_USER_KEY.map((b) => b.toString(16).padStart(2, "0")).join("");

const markTauri = () => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
};

const lastConfig = () => {
  const calls = notifyMocks.configure.mock.calls as unknown as Array<
    [import("../../domain/notify-client").NotifyConfigPayload]
  >;
  return calls[calls.length - 1]?.[0];
};

describe("desktop notify config push", () => {
  beforeEach(() => {
    localStorage.clear(); // a persisted viewMode would move the boot screen
    notifyMocks.configure.mockClear();
    notifyMocks.markSeen.mockClear();
  });
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    tauriEvent.handlers.clear();
  });

  it("stays silent on web — no Tauri, no push", async () => {
    const { transport } = makeFakeNode({ users: [SELF_ACCOUNT], publicKey: "AA" });
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );
    expect(notifyMocks.configure).not.toHaveBeenCalled();
  });

  it("pushes a payload whose self identity derives from nodeUsers[..].accountId", async () => {
    markTauri();
    const { transport } = makeFakeNode({ users: [SELF_ACCOUNT], publicKey: "AA" });
    renderConsole(transport);

    await waitFor(() => {
      const config = lastConfig();
      expect(config).toMatchObject({
        selfUserKeyHex: SELF_ACCOUNT_HEX,
        focusedChannel: "general",
      });
      // every node of the account, lowercase, self included
      expect(config!.selfNodeKeysHex).toEqual(["aa", "bb"]);
      expect(config!.prefs.enabled).toBe(true);
      expect(config!.authorNames["aa"]).toBe("Jess K");
    });
  });

  it("re-pushes on channel and screen switches with the new focusedChannel", async () => {
    markTauri();
    const { transport } = makeFakeNode({ users: [SELF_ACCOUNT], publicKey: "AA" });
    renderConsole(transport);
    await waitFor(() =>
      expect(lastConfig()).toMatchObject({ focusedChannel: "general" }),
    );

    await act(async () => {
      capturedActions!.createChannel("Dev Room", "open");
    });
    await waitFor(() =>
      expect(lastConfig()).toMatchObject({ focusedChannel: "dev-room" }),
    );

    // off the chat screen there is no focused channel to suppress
    await act(async () => {
      capturedActions!.setScreen("members");
    });
    await waitFor(() => expect(lastConfig()).toMatchObject({ focusedChannel: null }));
  });

  it("dedupes identical payloads across a refresh's identity churn", async () => {
    markTauri();
    const { transport, emitOps } = makeFakeNode({ users: [SELF_ACCOUNT], publicKey: "AA" });
    renderConsole(transport);
    await waitFor(() =>
      expect(lastConfig()).toMatchObject({ focusedChannel: "general" }),
    );

    const calls = notifyMocks.configure.mock.calls.length;
    // an identity-module block scopes the hydrate to the people slices: fresh
    // Record identities, identical values — the fingerprint must swallow the
    // re-run.
    const peopleQueries = vi
      .mocked(transport.query)
      .mock.calls.filter(([target]) => target === "identity").length;
    await act(async () => {
      emitOps("identity", [{ height: 5 }]);
      await new Promise((resolve) => setTimeout(resolve, 150));
    });
    await waitFor(() =>
      expect(
        vi.mocked(transport.query).mock.calls.filter(([target]) => target === "identity")
          .length,
      ).toBeGreaterThan(peopleQueries),
    );
    expect(notifyMocks.configure.mock.calls.length).toBe(calls);
  });

  it("tracks window focus in the config without marking seen", async () => {
    markTauri();
    const { transport } = makeFakeNode({ users: [SELF_ACCOUNT], publicKey: "AA" });
    renderConsole(transport);
    await waitFor(() =>
      expect(lastConfig()).toMatchObject({ focusedChannel: "general" }),
    );

    await act(async () => {
      window.dispatchEvent(new Event("blur"));
    });
    await waitFor(() => expect(lastConfig()).toMatchObject({ mainWindowFocused: false }));

    notifyMocks.markSeen.mockClear();
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => expect(lastConfig()).toMatchObject({ mainWindowFocused: true }));
    // Seen-marking belongs to the bell dropdown, not the focus edge.
    expect(notifyMocks.markSeen).not.toHaveBeenCalled();
  });
});

describe("ducktape://navigate deep-link", () => {
  beforeEach(() => {
    localStorage.clear(); // a persisted viewMode would move the boot screen
    notifyMocks.configure.mockClear();
  });
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    tauriEvent.handlers.clear();
  });

  it("keeps the plain-string screen switch byte-for-byte (tray popover)", async () => {
    markTauri();
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      tauriEvent.emitTo("ducktape://navigate", "members");
    });
    expect(capturedState!.screen).toBe("members");
  });

  it("navigates a structured chat target: screen, channel, thread", async () => {
    markTauri();
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );
    await act(async () => {
      capturedActions!.setScreen("members");
    });

    await act(async () => {
      tauriEvent.emitTo("ducktape://navigate", {
        screen: "chat",
        channelId: "dev",
        threadRoot: 7,
      });
    });

    await waitFor(() => {
      expect(capturedState!.screen).toBe("chat");
      expect(capturedState!.activeChannel).toBe("dev");
      expect(screen.getByTestId("thread").textContent).toBe("open");
    });
    // the thread was fetched from the DEEP-LINKED channel, not the one that
    // was active when the event arrived
    expect(vi.mocked(transport.query)).toHaveBeenCalledWith(
      "chat",
      expect.objectContaining({
        thread: expect.objectContaining({ channel_id: "dev", root_seq: 7 }),
      }),
    );
  });

  it("opens a thread in the already-active channel without a channel switch", async () => {
    markTauri();
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      tauriEvent.emitTo("ducktape://navigate", { screen: "chat", threadRoot: 1 });
    });

    await waitFor(() =>
      expect(screen.getByTestId("thread").textContent).toBe("open"),
    );
    expect(vi.mocked(transport.query)).toHaveBeenCalledWith(
      "chat",
      expect.objectContaining({
        thread: expect.objectContaining({ channel_id: "general", root_seq: 1 }),
      }),
    );
  });

  it("sets forgeFocus for a forge target and clears it on leaving the screen", async () => {
    markTauri();
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      tauriEvent.emitTo("ducktape://navigate", { screen: "forge", repo: "default", number: 7 });
    });
    await waitFor(() => {
      expect(capturedState!.screen).toBe("forge");
      expect(capturedState!.forgeFocus).toEqual({ repo: "default", number: 7 });
    });

    // the hand-off is one-shot: leaving the forge screen retires it, so a
    // later remount of the forge view can never replay the jump
    await act(async () => {
      tauriEvent.emitTo("ducktape://navigate", "members");
    });
    await waitFor(() => expect(capturedState!.forgeFocus).toBeNull());
  });

  it("reroutes a chat target into a hidden forge item channel to the forge view", async () => {
    // the run-finished deep-link for a forge-item mention targets screen
    // "chat" with the item's hidden `forge:<repo>:<n>` channel — unroutable
    // on the chat surface, so it must land on the item's forge view instead.
    markTauri();
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      tauriEvent.emitTo("ducktape://navigate", {
        screen: "chat",
        channelId: "forge:app:12",
        threadRoot: 7,
      });
    });
    await waitFor(() => {
      expect(capturedState!.screen).toBe("forge");
      expect(capturedState!.forgeFocus).toEqual({ repo: "app", number: 12 });
    });
    // no chat-side effects: the hidden channel is never entered.
    expect(capturedState!.activeChannel).toBe("general");
  });

  it("ignores malformed structured payloads", async () => {
    markTauri();
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );
    const before = capturedState!.screen;

    await act(async () => {
      tauriEvent.emitTo("ducktape://navigate", {});
      tauriEvent.emitTo("ducktape://navigate", { channelId: "dev" });
      tauriEvent.emitTo("ducktape://navigate", null);
      tauriEvent.emitTo("ducktape://navigate", 42);
    });
    expect(capturedState!.screen).toBe(before);
    expect(capturedState!.forgeFocus).toBeNull();
  });
});
