import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { MessageView } from "../../../domain/chat-client";
import { ConsoleContext } from "../../store/context";
import type { ConsoleActions } from "../../store/actions";
import { createInitialState, type ConsoleState } from "../../store/state";
import { ChatView } from "./ChatView";

const userAuthor = { user: Array.from(new TextEncoder().encode("operator")) };

const message = (
  seq: number,
  blocks: MessageView["head"]["blocks"],
  overrides: Partial<MessageView["head"]> = {},
): MessageView => ({
  channel_id: "general",
  seq,
  head: {
    message_id: `m-${seq}`,
    author: userAuthor,
    blocks,
    created_at: 1_735_689_600 + seq,
    rev: 1,
    edited_at: null,
    base_rev: null,
    deleted: false,
    thread: null,
    reply_count: 0,
    last_reply_seq: null,
    ...overrides,
  },
  reactions: [],
  channel_head_seq: 2,
});

const stateWithMessages = (messages: MessageView[]): ConsoleState => ({
  ...createInitialState(),
  connected: true,
  channels: [
    {
      id: "general",
      name: "general",
      created_at: 1,
      head_seq: messages.length,
      post_policy: "open",
      hooks: [],
      pinned: [],
    },
  ],
  activeChannel: "general",
  messages,
});

const noopActions = {
  selectChannel: vi.fn(),
  createChannel: vi.fn(),
  sendMessage: vi.fn(),
  openThread: vi.fn(),
  closeThread: vi.fn(),
  replyInThread: vi.fn(),
  toggleReaction: vi.fn(),
} as unknown as ConsoleActions;

describe("ChatView channel rail", () => {
  it("hides module-reserved channels (forge:* item threads) from the rail", () => {
    const base = stateWithMessages([]);
    render(
      <ConsoleContext.Provider
        value={{
          state: {
            ...base,
            channels: [
              ...base.channels,
              {
                id: "forge:ducktape:1",
                name: "ducktape#1",
                created_at: 2,
                head_seq: 0,
                post_policy: "open",
                hooks: [],
                pinned: [],
              },
            ],
          },
          actions: noopActions,
        }}
      >
        <ChatView />
      </ConsoleContext.Provider>,
    );

    // the user channel renders in the rail; the forge item's hidden discussion
    // channel must not (its messages belong to the forge view).
    expect(screen.getAllByText("general").length).toBeGreaterThan(0);
    expect(screen.queryByText("ducktape#1")).not.toBeInTheDocument();
  });
});

describe("ChatView layout", () => {
  it("keeps the message stream inset and confines long content to the chat column", () => {
    const longUrl = `https://example.test/${"very-long-segment".repeat(16)}`;
    const longCode = `const ${"token".repeat(18)} = "${"value".repeat(18)}";`;

    render(
      <ConsoleContext.Provider
        value={{
          state: stateWithMessages([
            message(1, [{ paragraph: [{ text: longUrl, marks: [] }] }]),
            message(2, [{ code: { lang: "ts", text: longCode } }]),
          ]),
          actions: noopActions,
        }}
      >
        <ChatView />
      </ConsoleContext.Provider>,
    );

    const stream = screen.getByRole("log", { name: "#general messages" });
    expect(stream).toHaveStyle({
      overflowX: "hidden",
      padding: "14px 18px 18px",
    });

    // The column fills the pane width (full-bleed rows / hover highlight, like
    // the reference) — long content is confined by the body's wrapping, asserted
    // below, not by a fixed max-width that would leave an awkward right gutter.
    const column = within(stream).getByTestId("chat-message-column");
    expect(column).toHaveStyle({
      width: "100%",
      minWidth: "0px",
    });

    const body = screen.getByText(longUrl).closest("[data-chat-body]");
    expect(body).toHaveStyle({
      overflowWrap: "anywhere",
      wordBreak: "break-word",
      maxWidth: "100%",
    });

    const codeBlock = screen.getByText(longCode);
    expect(codeBlock.tagName).toBe("PRE");
    expect(codeBlock).toHaveStyle({
      overflowX: "auto",
      maxWidth: "100%",
    });
  });
});
