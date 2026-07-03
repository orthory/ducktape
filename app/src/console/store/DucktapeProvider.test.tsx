// Provider contract over a fake transport: hydration projects node state in,
// writes submit the exact wire msg, and block events trigger a re-query.

import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { BlockEvent, NodeTransport } from "../../domain/transport";
import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";

// ── Fake node ───────────────────────────────────────────

const GENERAL_MESSAGE = {
  channel_id: "general",
  seq: 1,
  head: {
    message_id: "m1",
    author: { User: Array.from(new TextEncoder().encode("jess")) },
    blocks: [{ Paragraph: [{ text: "hello", marks: [] }] }],
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
  post_policy: "Open",
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
      const create = (payload as { CreateChannel?: { channel_id: string; name: string } })
        .CreateChannel;
      if (target === "chat" && create) {
        channels.push(wireChannel(create.channel_id, create.name, 2));
        messagesByChannel[create.channel_id] = [];
      }
      if (target === "forge" && (payload as { Commit?: unknown }).Commit) {
        forgeHead = "a".repeat(40);
      }
      return Promise.resolve({ height: 2, appHash: "bb".repeat(32) });
    }),
    query: vi.fn((target: string, query: unknown) => {
      if (target === "chat" && query === "Channels") {
        return Promise.resolve({ Channels: [...channels] });
      }
      const latest = (query as { MessagesLatest?: { channel_id: string } })
        .MessagesLatest;
      if (target === "chat" && latest) {
        return Promise.resolve({
          Messages: messagesByChannel[latest.channel_id] ?? [],
        });
      }
      if (target === "chat") {
        return Promise.resolve({
          Thread: { root: GENERAL_MESSAGE, replies: [] },
        });
      }
      if (target === "forge") {
        return Promise.resolve({ Head: forgeHead });
      }
      if (target === "agent") {
        if (query === "Agents") return Promise.resolve({ Agents: [] });
        if (query === "Watches") return Promise.resolve({ Watches: [] });
        return Promise.resolve({ Runs: [] });
      }
      if (target === "profiles") {
        return Promise.resolve({ Profiles: [] });
      }
      return Promise.resolve({ Tasks: [] });
    }),
    putBlob: vi.fn().mockResolvedValue("ab".repeat(32)),
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
    onTelemetry: vi.fn(() => () => {}),
  };
  const finalize = (block: BlockEvent) =>
    blockListeners.forEach((notify) => notify(block));
  return { transport, finalize };
};

let capturedActions: ConsoleActions | null = null;

function Probe() {
  const { state, actions } = useDucktape();
  capturedActions = actions;
  return (
    <div>
      <span data-testid="height">{state.status?.height ?? -1}</span>
      <span data-testid="channel">{state.activeChannel ?? "none"}</span>
      <span data-testid="messages">{state.messages.length}</span>
      <span data-testid="thread">{state.activeThread ? "open" : "closed"}</span>
      <span data-testid="forge">{state.forgeHead ?? "unborn"}</span>
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
      PostMessage: {
        channel_id: string;
        message_id: string;
        blocks: unknown[];
        thread: number | null;
      };
    };
    expect(msg.PostMessage.channel_id).toBe("general");
    expect(msg.PostMessage.blocks).toEqual([
      { Paragraph: [{ text: "hi node", marks: [] }] },
    ]);
    expect(msg.PostMessage.thread).toBeNull();
    expect(msg.PostMessage.message_id).toBeTruthy();
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
      capturedActions!.createChannel("Release Party");
    });

    await waitFor(() => {
      expect(screen.getByTestId("channel").textContent).toBe("release-party");
      // the stale-pane bug left #general's message list (1) showing here
      expect(screen.getByTestId("messages").textContent).toBe("0");
      expect(screen.getByTestId("thread").textContent).toBe("closed");
    });
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
      Commit: { path: "README.md", content: "hello forge", message: "init" },
    });
  });
});
