import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { Channel, MessageView } from "../../../domain/chat-client";
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
  setChannelArchived: vi.fn(),
} as unknown as ConsoleActions;

const archivedChannel = (id: string): Channel => ({
  id,
  name: id,
  created_at: 2,
  head_seq: 0,
  post_policy: "open",
  hooks: [],
  pinned: [],
  archived: true,
});

const renderChat = (state: ConsoleState, actions: ConsoleActions = noopActions) =>
  render(
    <ConsoleContext.Provider value={{ state, actions }}>
      <ChatView />
    </ConsoleContext.Provider>,
  );

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

describe("ChatView archived channels", () => {
  it("replaces the composer with an archived notice on the active channel", () => {
    const base = stateWithMessages([]);
    const setChannelArchived = vi.fn();
    renderChat(
      { ...base, channels: [{ ...base.channels[0]!, archived: true }] },
      { ...noopActions, setChannelArchived } as unknown as ConsoleActions,
    );

    // the composer is gone entirely — a post typed into it would be rejected by
    // the module — and the notice says why in its place.
    expect(screen.queryByPlaceholderText("Message #general")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Send message")).not.toBeInTheDocument();
    expect(screen.getByText(/This channel is archived/)).toBeInTheDocument();

    // an owner-less channel admits any user's admin op (check_channel_admin), so
    // the notice offers the way back out.
    fireEvent.click(screen.getByText("Unarchive"));
    expect(setChannelArchived).toHaveBeenCalledWith("general", false);
  });

  it("lists archived channels under the rail's Archived section and enters one on click", () => {
    const base = stateWithMessages([]);
    const selectChannel = vi.fn();
    renderChat(
      { ...base, channels: [...base.channels, archivedChannel("retro")] },
      { ...noopActions, selectChannel } as unknown as ConsoleActions,
    );

    // collapsed by default: the archived channel is not in the main list…
    expect(screen.queryByText("retro")).not.toBeInTheDocument();
    // …and the section that holds it counts what's inside.
    fireEvent.click(screen.getByText("ARCHIVED · 1"));

    fireEvent.click(screen.getByText("retro"));
    expect(selectChannel).toHaveBeenCalledWith("retro");
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

    // Code fences WRAP long lines (Slack behavior) — a horizontal scrollbar
    // inside a chat row hides content.
    const codeBlock = screen.getByText(longCode);
    expect(codeBlock.tagName).toBe("PRE");
    expect(codeBlock).toHaveStyle({
      whiteSpace: "pre-wrap",
      overflowWrap: "anywhere",
      maxWidth: "100%",
    });
  });
});

describe("ChatView jump-to-message", () => {
  const focusState = (overrides: Partial<ConsoleState> = {}): ConsoleState => ({
    ...stateWithMessages([message(900, [{ paragraph: [{ text: "tail", marks: [] }] }])]),
    // the hit is far older than the loaded tail slice — it has no row.
    chatFocusSeq: 12,
    ...overrides,
  });

  it("pages in the window around a focused seq that is older than the loaded tail", () => {
    const actions = { ...noopActions, loadMessageWindow: vi.fn(), clearChatFocus: vi.fn() };
    render(
      <ConsoleContext.Provider value={{ state: focusState(), actions }}>
        <ChatView />
      </ConsoleContext.Provider>,
    );

    expect(actions.loadMessageWindow).toHaveBeenCalledWith("general", 12);
  });

  it("asks only once — a window already loaded for that seq is not re-requested", () => {
    const actions = { ...noopActions, loadMessageWindow: vi.fn(), clearChatFocus: vi.fn() };
    render(
      <ConsoleContext.Provider
        value={{
          // the window came back but carries no row for the seq (an impossible
          // one, or a node that could not page it in): asking again would loop.
          state: focusState({ chatWindow: { channelId: "general", seq: 12 } }),
          actions,
        }}
      >
        <ChatView />
      </ConsoleContext.Provider>,
    );

    expect(actions.loadMessageWindow).not.toHaveBeenCalled();
    expect(screen.getByText("Jump to latest")).toBeInTheDocument();
  });
});
