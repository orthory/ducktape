import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AuthorRef, MessageView } from "../../../domain/chat-client";
import { selfAuthorKeyOf } from "./chat-helpers";
import { MessageItem } from "./MessageItem";

const SELF = "operator";
const selfKey = selfAuthorKeyOf(SELF);
const ownAuthor: AuthorRef = { user: Array.from(new TextEncoder().encode(SELF)) };
const otherAuthor: AuthorRef = { user: Array.from(new TextEncoder().encode("someone-else")) };

const msg = (author: AuthorRef, text: string, overrides: Partial<MessageView["head"]> = {}): MessageView => ({
  channel_id: "general",
  seq: 7,
  head: {
    message_id: "m-7",
    author,
    blocks: [{ paragraph: [{ text, marks: [] }] }],
    created_at: 1_735_689_600,
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
  channel_head_seq: 9,
});

const baseProps = {
  names: {},
  groupStart: true,
  selfKey,
  hovered: false,
  menuOpen: true,
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

describe("MessageItem edit/delete", () => {
  it("offers Edit/Delete only on the author's own message", () => {
    const { unmount } = render(<MessageItem {...baseProps} message={msg(ownAuthor, "hi team")} />);
    expect(screen.getByText("Edit message")).toBeTruthy();
    expect(screen.getByText("Delete message")).toBeTruthy();
    unmount();

    render(<MessageItem {...baseProps} message={msg(otherAuthor, "hi team")} />);
    expect(screen.queryByText("Edit message")).toBeNull();
    expect(screen.queryByText("Delete message")).toBeNull();
  });

  it("never offers edit/delete on a tombstoned message", () => {
    render(<MessageItem {...baseProps} message={msg(ownAuthor, "", { deleted: true })} />);
    expect(screen.queryByText("Edit message")).toBeNull();
    expect(screen.queryByText("Delete message")).toBeNull();
  });

  it("badges an agent author with the AGENT pill (wire casing is lowercase `agent`)", () => {
    const agentAuthor: AuthorRef = { agent: { module: "automations", agent_id: "helper" } };
    const { unmount } = render(<MessageItem {...baseProps} message={msg(agentAuthor, "on it")} />);
    expect(screen.getByText("AGENT")).toBeTruthy();
    unmount();

    // a plain user author must NOT be badged
    render(<MessageItem {...baseProps} message={msg(otherAuthor, "hi")} />);
    expect(screen.queryByText("AGENT")).toBeNull();
  });

  it("opens an inline editor seeded with the message text and saves the new text", () => {
    const onEdit = vi.fn();
    render(<MessageItem {...baseProps} onEdit={onEdit} message={msg(ownAuthor, "original")} />);

    fireEvent.click(screen.getByText("Edit message"));
    const editor = screen.getByRole("textbox") as HTMLTextAreaElement;
    expect(editor.value).toBe("original");

    fireEvent.change(editor, { target: { value: "edited body" } });
    fireEvent.click(screen.getByText("Save"));
    expect(onEdit).toHaveBeenCalledWith("edited body");
  });

  it("seeds the inline editor with re-editable source for a code-block message", () => {
    const codeMessage = msg(ownAuthor, "", { blocks: [{ code: { lang: "ts", text: "const a = 1;" } }] });
    render(<MessageItem {...baseProps} message={codeMessage} />);

    fireEvent.click(screen.getByText("Edit message"));
    const editor = screen.getByRole("textbox") as HTMLTextAreaElement;
    expect(editor.value).toContain("```ts");
    expect(editor.value).toContain("const a = 1;");
  });

  it("requires a confirm step before deleting", () => {
    const onDelete = vi.fn();
    render(<MessageItem {...baseProps} onDelete={onDelete} message={msg(ownAuthor, "bye")} />);

    fireEvent.click(screen.getByText("Delete message"));
    expect(screen.getByText(/Delete this message/)).toBeTruthy();
    expect(onDelete).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Delete", { selector: "button" }));
    expect(onDelete).toHaveBeenCalledTimes(1);
  });
});

describe("MessageItem #tags", () => {
  it("makes a grammar-valid #tag clickable when onTagClick is wired", () => {
    const onTagClick = vi.fn();
    render(
      <MessageItem
        {...baseProps}
        onTagClick={onTagClick}
        message={msg(otherAuthor, "shipping #Rust-지원 today, not foo#bar")}
      />,
    );

    // the tag (without its trailing prose) is a button; clicking hands the
    // label over WITHOUT the leading #.
    fireEvent.click(screen.getByRole("button", { name: "#Rust-지원" }));
    expect(onTagClick).toHaveBeenCalledWith("Rust-지원");
    // a mid-word # never becomes an affordance.
    expect(screen.queryByRole("button", { name: /#bar/ })).toBeNull();
  });

  it("keeps #tags inert (tinted text, no button) without onTagClick", () => {
    render(<MessageItem {...baseProps} message={msg(otherAuthor, "plain #tag here")} />);
    expect(screen.getByText("#tag")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "#tag" })).toBeNull();
  });
});
