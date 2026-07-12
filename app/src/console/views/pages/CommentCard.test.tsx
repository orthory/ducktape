import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { CommentCard } from "./CommentCard";
import type { ThreadView } from "../../../domain/pages-client";
import type { AuthorRef } from "../../../domain/chat-client";

const alice: AuthorRef = { user: [1] };

const thread: ThreadView = {
  thread: {
    id: "t1",
    target: "b1",
    opener: alice,
    created_at: 1,
    resolved: false,
    resolved_by: null,
    comment_ids: ["c1"],
  },
  comments: [
    {
      id: "c1",
      thread_id: "t1",
      author: alice,
      text: "hello world",
      created_at: 1,
      edited_at: null,
      deleted: false,
    },
  ],
};

const renderCard = (over: Partial<Parameters<typeof CommentCard>[0]> = {}) => {
  const props = {
    target: "b1",
    label: "this block",
    anchor: { x: 400, y: 300 },
    threads: [] as ThreadView[],
    authorNames: {},
    // alice IS us: authorKey({ user: [1] }) === "user:1".
    selfKey: "user:1",
    onClose: vi.fn(),
    onSubmitNew: vi.fn(),
    onReply: vi.fn(),
    onResolve: vi.fn(),
    onEdit: vi.fn(),
    onDelete: vi.fn(),
    ...over,
  };
  return { ...render(<CommentCard {...props} />), props };
};

describe("CommentCard", () => {
  it("renders a fixed dialog named for its target", () => {
    const { getByRole } = renderCard();
    const dialog = getByRole("dialog", { name: "Comments on this block" });
    expect(dialog.style.position).toBe("fixed");
  });

  it("opens straight into the composer when the target has no threads", () => {
    const { getByLabelText, getByRole, props } = renderCard();
    fireEvent.change(getByLabelText("New comment text"), {
      target: { value: "first!" },
    });
    getByRole("button", { name: /add comment/i }).click();
    expect(props.onSubmitNew).toHaveBeenCalledWith("b1", "first!");
    // the empty-target composer's Cancel dismisses the whole card.
    getByRole("button", { name: /cancel new comment/i }).click();
    expect(props.onClose).toHaveBeenCalled();
  });

  it("lists existing threads and hides the composer behind Add comment", () => {
    const { getByText, getByRole, getByLabelText, queryByLabelText, props } = renderCard({
      threads: [thread],
    });
    getByText("hello world");
    expect(queryByLabelText("New comment text")).toBeNull();

    fireEvent.click(getByRole("button", { name: "Add comment thread" }));
    fireEvent.change(getByLabelText("New comment text"), {
      target: { value: "another" },
    });
    fireEvent.click(getByRole("button", { name: "Add comment" }));
    expect(props.onSubmitNew).toHaveBeenCalledWith("b1", "another");

    // with threads present, Cancel only hides the composer — card stays.
    fireEvent.click(getByRole("button", { name: "Add comment thread" }));
    fireEvent.click(getByRole("button", { name: /cancel new comment/i }));
    expect(props.onClose).not.toHaveBeenCalled();
    expect(queryByLabelText("New comment text")).toBeNull();
  });

  it("relays reply and resolve to the thread card", () => {
    const { getByLabelText, getByRole, props } = renderCard({ threads: [thread] });
    fireEvent.change(getByLabelText("Reply to thread"), {
      target: { value: "yes" },
    });
    fireEvent.keyDown(getByLabelText("Reply to thread"), { key: "Enter" });
    expect(props.onReply).toHaveBeenCalledWith("t1", "yes");
    getByRole("button", { name: /resolve thread/i }).click();
    expect(props.onResolve).toHaveBeenCalledWith("t1", true);
  });

  it("closes on Escape and on outside mousedown, not inside", () => {
    const { getByRole, props } = renderCard({ threads: [thread] });
    fireEvent.mouseDown(getByRole("dialog"));
    expect(props.onClose).not.toHaveBeenCalled();
    fireEvent.mouseDown(document.body);
    expect(props.onClose).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(props.onClose).toHaveBeenCalledTimes(2);
  });

  it("closes when the document scrolls, but not on its own scroll", () => {
    const { getByRole, props } = renderCard({ threads: [thread] });
    fireEvent.scroll(getByRole("dialog"));
    expect(props.onClose).not.toHaveBeenCalled();
    fireEvent.scroll(document);
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  // The module enforces author-only edit/delete. The buttons used to render on
  // EVERY comment, so a click on someone else's bought a rejected op and an
  // error — never an edit.
  it("offers Edit/Delete only on our own comments", () => {
    const bob: AuthorRef = { user: [2] };
    const mixed: ThreadView = {
      ...thread,
      comments: [
        thread.comments[0],
        { ...thread.comments[0], id: "c2", author: bob, text: "not mine" },
      ],
    };
    const { getAllByRole } = renderCard({ threads: [mixed], selfKey: "user:1" });
    expect(getAllByRole("button", { name: "Edit comment" })).toHaveLength(1);
    expect(getAllByRole("button", { name: "Delete comment" })).toHaveLength(1);
  });

  it("offers neither when nothing in the thread is ours", () => {
    const { queryByRole } = renderCard({ threads: [thread], selfKey: "user:99" });
    expect(queryByRole("button", { name: "Edit comment" })).toBeNull();
    expect(queryByRole("button", { name: "Delete comment" })).toBeNull();
  });

  it("replies through a multi-line textarea, like the new-thread composer", () => {
    const { getByLabelText, props } = renderCard({ threads: [thread] });
    const reply = getByLabelText("Reply to thread");
    expect(reply.tagName).toBe("TEXTAREA");
    fireEvent.change(reply, { target: { value: "line one" } });
    // Shift+Enter breaks a line instead of sending — the composer's grammar.
    fireEvent.keyDown(reply, { key: "Enter", shiftKey: true });
    expect(props.onReply).not.toHaveBeenCalled();
    fireEvent.keyDown(reply, { key: "Enter" });
    expect(props.onReply).toHaveBeenCalledWith("t1", "line one");
  });
});
