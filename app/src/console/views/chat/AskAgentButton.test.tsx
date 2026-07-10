// The per-message "ask an agent to respond" hover action, exercised through
// the REAL MessageItem hover bar: shown only with ≥1 Active agent, opens the
// popover, and a pick submits requestRun anchored on THIS message's seq.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AgentRecord } from "../../../domain/agent-client";
import type { AuthorRef, MessageView } from "../../../domain/chat-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState } from "../../store/state";
import { selfAuthorKeyOf } from "./chat-helpers";
import { MessageItem } from "./MessageItem";

const agent = (
  agentId: string,
  status: AgentRecord["status"] = "active",
): AgentRecord => ({
  agent_id: agentId,
  owner: { external: [1] },
  display_name: agentId,
  capability: "echo",
  prompt_hash: Array(32).fill(7),
  allowed_actions: ["chat.post"],
  status,
  created_at: 1,
  updated_at: 1,
});

const author: AuthorRef = { user: Array.from(new TextEncoder().encode("someone")) };

const message: MessageView = {
  channel_id: "general",
  seq: 7,
  head: {
    message_id: "m-7",
    author,
    blocks: [{ paragraph: [{ text: "hi team", marks: [] }] }],
    created_at: 1_735_689_600,
    rev: 1,
    edited_at: null,
    base_rev: null,
    deleted: false,
    thread: null,
    reply_count: 0,
    last_reply_seq: null,
  },
  reactions: [],
  channel_head_seq: 9,
};

const rowProps = {
  message,
  names: {},
  groupStart: true,
  selfKey: selfAuthorKeyOf(Array.from(new TextEncoder().encode("operator"))),
  hovered: true,
  menuOpen: false,
  replyHint: null,
  linkRef: "ducktape://local/general/m-7",
  refRef: "general#7:m-7",
  onHover: vi.fn(),
  onMenuToggle: vi.fn(),
  onOpenThread: vi.fn(),
  onReact: vi.fn(),
  onEdit: vi.fn(),
  onDelete: vi.fn(),
};

const renderRow = (agents: AgentRecord[], requestRun = vi.fn()) => {
  render(
    <ConsoleContext.Provider
      value={{
        state: { ...createInitialState(), agents },
        actions: { requestRun } as unknown as ConsoleActions,
      }}
    >
      <MessageItem {...rowProps} />
    </ConsoleContext.Provider>,
  );
  return requestRun;
};

describe("AskAgentButton", () => {
  it("submits requestRun anchored on the message's seq when an agent is picked", () => {
    const requestRun = renderRow([agent("quackbot"), agent("scribe")]);
    fireEvent.click(screen.getByTitle("Ask an agent to respond"));
    expect(screen.getByText("ASK TO RESPOND")).toBeTruthy();

    fireEvent.click(screen.getByText("@scribe"));
    expect(requestRun).toHaveBeenCalledWith({
      agentId: "scribe",
      channelId: "general",
      anchorSeq: 7,
    });
    expect(screen.queryByText("ASK TO RESPOND")).toBeNull();
  });

  it("offers nothing when no agent is Active", () => {
    renderRow([agent("idler", "paused")]);
    expect(screen.queryByTitle("Ask an agent to respond")).toBeNull();
  });

  it("offers nothing outside a store context (bare component render)", () => {
    render(<MessageItem {...rowProps} />);
    expect(screen.queryByTitle("Ask an agent to respond")).toBeNull();
  });
});
