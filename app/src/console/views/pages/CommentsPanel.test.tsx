import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
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
        onClose={vi.fn()}
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
