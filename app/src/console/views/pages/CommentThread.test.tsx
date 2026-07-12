// The comment composers' @mention typeahead (the chat menu, portaled), the
// keys it owns while open, and the thread card's identity/status texture
// (timestamps, resolved chip).

import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { AgentRecord } from "../../../domain/agent-client";
import type { AuthorRef } from "../../../domain/chat-client";
import type { ThreadView } from "../../../domain/pages-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { NewThreadComposer, ThreadCard } from "./CommentThread";

const QUACKBOT: AgentRecord = {
  agent_id: "quackbot",
  owner: { external: [1] },
  display_name: "Quackbot",
  capability: "echo",
  allowed_actions: ["chat.post"],
  status: "active",
  created_at: 1,
  updated_at: 1,
};

const alice: AuthorRef = { user: [1] };

const view = (over: Partial<ThreadView["thread"]> = {}, createdAt = 1): ThreadView => ({
  thread: {
    id: "t1",
    target: "b1",
    opener: alice,
    created_at: createdAt,
    resolved: false,
    resolved_by: null,
    comment_ids: ["c1"],
    ...over,
  },
  comments: [
    {
      id: "c1",
      thread_id: "t1",
      author: alice,
      text: "hello world",
      created_at: createdAt,
      edited_at: null,
      deleted: false,
    },
  ],
});

const withStore = (node: ReactNode, statePatch: Partial<ConsoleState> = {}) =>
  render(
    <ConsoleContext.Provider
      value={{
        state: { ...createInitialState(), agents: [QUACKBOT], ...statePatch },
        actions: {} as ConsoleActions,
      }}
    >
      {node}
    </ConsoleContext.Provider>,
  );

describe("comment composer @mention typeahead", () => {
  it("opens on an @token, and Enter picks instead of submitting", () => {
    const onSubmit = vi.fn();
    withStore(
      <NewThreadComposer
        composer={{ target: "b1", label: "this block" }}
        onSubmit={onSubmit}
        onCancel={vi.fn()}
      />,
    );
    const input = screen.getByLabelText("New comment text") as HTMLTextAreaElement;
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "@qu" } });

    // the menu portals to document.body (the card/panel clip overflow)
    expect(screen.getByRole("listbox", { name: /mention/i })).toBeTruthy();

    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
    expect(input.value).toBe("@quackbot ");
    // picked — the token is gone, so the menu is too
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("Escape dismisses the menu without cancelling the composer", () => {
    const onCancel = vi.fn();
    withStore(
      <NewThreadComposer
        composer={{ target: "b1", label: "this block" }}
        onSubmit={vi.fn()}
        onCancel={onCancel}
      />,
    );
    const input = screen.getByLabelText("New comment text");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "@qu" } });
    fireEvent.keyDown(input, { key: "Escape" });

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onCancel).not.toHaveBeenCalled();
    // the NEXT Escape (menu closed) cancels as before
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onCancel).toHaveBeenCalled();
  });

  it("the reply box picks on Enter too, never posting the raw token", () => {
    const onReply = vi.fn();
    withStore(
      <ThreadCard
        view={view()}
        authorNames={{}}
        selfKey="user:1"
        onReply={onReply}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    const input = screen.getByLabelText("Reply to thread") as HTMLTextAreaElement;
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "@qu" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onReply).not.toHaveBeenCalled();
    expect(input.value).toBe("@quackbot ");

    fireEvent.keyDown(input, { key: "Enter" });
    expect(onReply).toHaveBeenCalledWith("t1", "@quackbot ");
  });

  it("closes when the textarea loses focus — no orphaned menu over other composers", () => {
    withStore(
      <NewThreadComposer
        composer={{ target: "b1", label: "this block" }}
        onSubmit={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    const input = screen.getByLabelText("New comment text");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "@qu" } });
    expect(screen.getByRole("listbox", { name: /mention/i })).toBeTruthy();

    fireEvent.blur(input);
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("degrades to a plain textarea without a store (no menu, no crash)", () => {
    render(
      <NewThreadComposer
        composer={{ target: "b1", label: "this block" }}
        onSubmit={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    const bare = screen.getByLabelText("New comment text");
    fireEvent.focus(bare);
    fireEvent.change(bare, { target: { value: "@qu" } });
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});

describe("thread card texture", () => {
  it("stamps a wall-clock comment time, and stays silent pre-wall-clock", () => {
    const at = new Date("2026-03-05T12:00:00Z").getTime();
    const expected = new Date(at).toLocaleDateString([], { month: "short", day: "numeric" });
    const { container, unmount } = render(
      <ThreadCard
        view={view({}, at)}
        authorNames={{}}
        selfKey="user:1"
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(container.textContent).toContain(expected);
    unmount();

    // created_at 1 is genesis-relative — no fake "Jan 1, 1970"
    const bare = render(
      <ThreadCard
        view={view({}, 1)}
        authorNames={{}}
        selfKey="user:1"
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(bare.container.textContent).not.toContain("1970");
  });

  it("shows who resolved a thread and offers Reopen", () => {
    render(
      <ThreadCard
        view={view({ resolved: true, resolved_by: alice })}
        authorNames={{ "01": "Alice" }}
        selfKey="user:1"
        onReply={vi.fn()}
        onResolve={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByText(/Resolved by Alice/)).toBeTruthy();
    expect(screen.getByRole("button", { name: /reopen thread/i })).toBeTruthy();
  });
});
