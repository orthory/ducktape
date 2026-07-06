import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { CommentsPanel } from "./CommentsPanel";
import type { AnchorThreads } from "../../../domain/comments-client";
import type { AuthorRef } from "../../../domain/chat-client";

const alice: AuthorRef = { user: [1] };

const threads: AnchorThreads[] = [
  {
    target: "b1",
    threads: [
      {
        thread: {
          id: "t1",
          anchor: { module: "pages", target: "b1" },
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
        onClose={vi.fn()}
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

  it("shows an empty state with no threads", () => {
    const { getByText } = render(
      <CommentsPanel
        threads={[]}
        authorNames={{}}
        onClose={vi.fn()}
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    getByText(/no comments yet/i);
  });
});
