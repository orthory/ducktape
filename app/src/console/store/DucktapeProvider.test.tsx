// Provider contract over a fake transport: hydration projects node state in,
// writes submit the exact wire msg, and block events trigger a re-query.

import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { BlockEvent, NodeTransport } from "../../domain/transport";
import { DucktapeProvider } from "./DucktapeProvider";
import { useDucktape } from "./use-ducktape";
import type { ConsoleActions } from "./DucktapeProvider";

// ── Fake node ───────────────────────────────────────────

const makeFakeNode = () => {
  const blockListeners = new Set<(block: BlockEvent) => void>();
  const transport: NodeTransport = {
    submit: vi.fn().mockResolvedValue({ height: 2, appHash: "bb".repeat(32) }),
    query: vi.fn((target: string, query: unknown) => {
      if (target === "chat" && query === "Channels") {
        return Promise.resolve({
          Channels: [{ id: "general", name: "General", created_at: 1 }],
        });
      }
      if (target === "chat") {
        return Promise.resolve({
          Messages: [
            {
              id: "m1",
              channel_id: "general",
              author: "jess",
              body: "hello",
              sequence: 1,
              sent_at: 10,
              thread_id: null,
              reply_count: 0,
              last_reply_at: null,
            },
          ],
        });
      }
      return Promise.resolve({ Tasks: [] });
    }),
    status: vi.fn().mockResolvedValue({
      appHash: "aa".repeat(32),
      height: 1,
      modules: [{ id: "chat", root: "cc".repeat(32) }],
    }),
    onBlock: vi.fn((listener: (block: BlockEvent) => void) => {
      blockListeners.add(listener);
      return () => blockListeners.delete(listener);
    }),
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

  it("sendMessage submits the wire msg with the local author", async () => {
    const { transport } = makeFakeNode();
    renderConsole(transport);
    await waitFor(() =>
      expect(screen.getByTestId("channel").textContent).toBe("general"),
    );

    await act(async () => {
      capturedActions!.sendMessage("hi node");
    });

    await waitFor(() => expect(transport.submit).toHaveBeenCalled());
    const [target, payload] = vi.mocked(transport.submit).mock.calls[0];
    expect(target).toBe("chat");
    const msg = payload as { SendMessage: Record<string, string> };
    expect(msg.SendMessage.channel_id).toBe("general");
    expect(msg.SendMessage.author).toBe("operator");
    expect(msg.SendMessage.body).toBe("hi node");
    expect(msg.SendMessage.message_id).toBeTruthy();
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
});
