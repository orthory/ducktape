import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { CommentsPanel } from "./CommentsPanel";
import type { TargetThreads } from "../../../domain/pages-client";
import type { AuthorRef } from "../../../domain/chat-client";

const alice: AuthorRef = { user: [1] };

const threads: TargetThreads[] = [
  {
    target: "b1",
    threads: [
      {
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
      },
    ],
  },
];

describe("CommentsPanel", () => {
  it("lists thread comments and resolves", () => {
    const onResolve = vi.fn();
    const { getByText, getByRole } = render(
      <CommentsPanel
        threads={threads}
        authorNames={{}}
        selfKey="user:1"
        composer={null}
        onClose={vi.fn()}
        onSubmitNew={vi.fn()}
        onCancelNew={vi.fn()}
        onReply={vi.fn()}
        onResolve={onResolve}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    getByText("hello world");
    getByRole("button", { name: /resolve thread/i }).click();
    expect(onResolve).toHaveBeenCalledWith("t1", true);
  });

  it("sinks resolved threads below open ones", () => {
    const two: TargetThreads[] = [
      {
        target: "b1",
        threads: [
          {
            thread: {
              id: "t-done",
              target: "b1",
              opener: alice,
              created_at: 1,
              resolved: true,
              resolved_by: alice,
              comment_ids: ["c-done"],
            },
            comments: [
              {
                id: "c-done",
                thread_id: "t-done",
                author: alice,
                text: "settled thing",
                created_at: 1,
                edited_at: null,
                deleted: false,
              },
            ],
          },
          ...threads[0].threads,
        ],
      },
    ];
    const { container } = render(
      <CommentsPanel
        threads={two}
        authorNames={{}}
        selfKey="user:1"
        composer={null}
        onClose={vi.fn()}
        onSubmitNew={vi.fn()}
        onCancelNew={vi.fn()}
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    const text = container.textContent ?? "";
    expect(text.indexOf("hello world")).toBeGreaterThan(-1);
    expect(text.indexOf("hello world")).toBeLessThan(text.indexOf("settled thing"));
  });

  it("shows an empty state with no threads", () => {
    const { getByText } = render(
      <CommentsPanel
        threads={[]}
        authorNames={{}}
        selfKey="user:1"
        composer={null}
        onClose={vi.fn()}
        onSubmitNew={vi.fn()}
        onCancelNew={vi.fn()}
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    getByText(/no comments yet/i);
  });

  it("submits a new thread from the composer", () => {
    const onSubmitNew = vi.fn();
    const { getByLabelText, getByRole, queryByText } = render(
      <CommentsPanel
        threads={[]}
        authorNames={{}}
        selfKey="user:1"
        composer={{ target: "b1", label: "this block" }}
        onClose={vi.fn()}
        onSubmitNew={onSubmitNew}
        onCancelNew={vi.fn()}
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    // composer replaces the empty-state hint while it is open.
    expect(queryByText(/no comments yet/i)).toBeNull();
    fireEvent.change(getByLabelText("New comment text"), {
      target: { value: "first!" },
    });
    getByRole("button", { name: /add comment/i }).click();
    expect(onSubmitNew).toHaveBeenCalledWith("b1", "first!");
  });

  it("ignores submit with only whitespace and cancels", () => {
    const onSubmitNew = vi.fn();
    const onCancelNew = vi.fn();
    const { getByLabelText, getByRole } = render(
      <CommentsPanel
        threads={[]}
        authorNames={{}}
        selfKey="user:1"
        composer={{ target: "p1", label: "this page" }}
        onClose={vi.fn()}
        onSubmitNew={onSubmitNew}
        onCancelNew={onCancelNew}
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    fireEvent.change(getByLabelText("New comment text"), {
      target: { value: "   " },
    });
    getByRole("button", { name: /add comment/i }).click();
    expect(onSubmitNew).not.toHaveBeenCalled();
    getByRole("button", { name: /cancel new comment/i }).click();
    expect(onCancelNew).toHaveBeenCalled();
  });
});
