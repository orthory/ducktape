import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { Notification } from "../../../domain/inbox-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { InboxView } from "./InboxView";

const notifications: Notification[] = [
  {
    seq: 1,
    member: "operator",
    kind: "mention",
    body: "You were mentioned in #general",
    source: "chat",
    created_at: 1_700_000_000,
    read: true,
  },
  {
    seq: 2,
    member: "operator",
    kind: "task",
    body: "Task assigned to you",
    source: "tasks",
    created_at: 1_700_003_600,
    read: false,
  },
];

const renderInbox = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    status: {
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 8,
      modules: [{ id: "inbox", root: "bb".repeat(32) }],
    },
    inbox: notifications,
    inboxUnread: 1,
    ...patch,
  };
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const noop = vi.fn() as (...args: unknown[]) => void;

  function Harness() {
    const [state, setState] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn() as (...args: unknown[]) => void;
          if (key === "deliverNotification") {
            return (params: unknown) => {
              spies[key](params);
              setState((prev) => ({ ...prev, inbox: prev.inbox }));
            };
          }
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <InboxView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

describe("InboxView", () => {
  it("shows the unread pill and marks the whole queue read", () => {
    const { spies } = renderInbox();

    expect(screen.getByText("1 unread")).toBeInTheDocument();
    expect(screen.getByText("You were mentioned in #general")).toBeInTheDocument();
    expect(screen.getByText("Task assigned to you")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /mark all read/i }));
    expect(spies.markInboxRead).toHaveBeenCalled();
  });

  it("is honest when the inbox module is not backed by the node", () => {
    renderInbox({
      inbox: [],
      inboxUnread: 0,
      status: {
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: 8,
        modules: [{ id: "chat", root: "bb".repeat(32) }],
      },
    });

    expect(screen.getByText(/inbox module is not available/i)).toBeInTheDocument();
  });
});
