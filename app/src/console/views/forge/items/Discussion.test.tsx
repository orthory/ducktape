import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { MessageView } from "../../../../domain/chat-client";
import { makeTransportStub } from "../../../../test/transport-stub";
import type { ConsoleActions } from "../../../store/actions";
import { ConsoleContext } from "../../../store/context";
import { createInitialState } from "../../../store/state";
import { Discussion } from "./Discussion";

const message = (seq: number, id: string): MessageView => ({
  channel_id: "forge:ducktape:58",
  seq,
  head: {
    message_id: id,
    author: { user: [1] },
    blocks: [{ paragraph: [{ text: `message ${seq}`, marks: [] }] }],
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
  channel_head_seq: seq,
});

const renderDiscussion = (props: { messageId?: string; messageSeq?: number }) => {
  const transport = makeTransportStub({
    query: vi.fn().mockResolvedValue({ messages: [message(3, "m3"), message(4, "m4")] }),
  });
  render(
    <ConsoleContext.Provider
      value={{
        state: { ...createInitialState(), connected: true },
        actions: { postInChannel: vi.fn() } as unknown as ConsoleActions,
        transport,
      }}
    >
      <Discussion channelId="forge:ducktape:58" {...props} />
    </ConsoleContext.Provider>,
  );
  return transport;
};

describe("Discussion anchors", () => {
  it("focuses the matching message id after the discussion loads", async () => {
    const scrollIntoView = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    renderDiscussion({ messageId: "m4" });

    const row = await waitFor(() => {
      const next = document.getElementById("forge-discussion-message-4");
      expect(next).toHaveFocus();
      return next;
    });
    expect(row).toHaveAttribute("data-message-id", "m4");
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "center" });
  });

  it("keeps the parent item usable when an anchored message is missing", async () => {
    renderDiscussion({ messageSeq: 99 });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Referenced message is unavailable. Showing the Forge item instead.",
    );
    expect(screen.getByText("message 3")).toBeInTheDocument();
  });
});
