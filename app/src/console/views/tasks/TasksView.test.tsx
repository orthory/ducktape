import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { Task } from "../../../domain/tasks-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { TasksView } from "./TasksView";

const tasks: Task[] = [
  {
    id: "task-open-123456",
    title: "Review launch checklist",
    status: "open",
    created_at: 1_700_000_000,
    updated_at: 1_700_000_000,
  },
  {
    id: "task-progress-123456",
    title: "Ship release notes",
    status: "in_progress",
    created_at: 1_700_003_600,
    updated_at: 1_700_003_600,
  },
  {
    id: "task-done-123456",
    title: "Archive old plan",
    status: "done",
    created_at: 1_700_007_200,
    updated_at: 1_700_007_200,
  },
];

const renderTasks = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    status: {
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 8,
      modules: [{ id: "tasks", root: "bb".repeat(32) }],
    },
    tasks,
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
          if (key === "addTask") {
            return (title: string) => {
              spies[key](title);
              setState((prev) => ({ ...prev, tasks: prev.tasks }));
            };
          }
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <TasksView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

describe("TasksView", () => {
  it("submits the labeled composer from the keyboard and exposes explicit advance controls", () => {
    const { spies } = renderTasks();

    expect(screen.getByText("1 open")).toBeInTheDocument();
    expect(screen.getByText("1 in progress")).toBeInTheDocument();
    expect(screen.getByText("1 done")).toBeInTheDocument();

    const input = screen.getByLabelText(/task title/i);
    fireEvent.change(input, { target: { value: "  Cut release branch  " } });
    fireEvent.submit(input.closest("form")!);

    expect(spies.addTask).toHaveBeenCalledWith("Cut release branch");
    expect(input).toHaveValue("");

    fireEvent.click(screen.getByRole("button", { name: /start review launch checklist/i }));
    expect(spies.advanceTask).toHaveBeenCalledWith("task-open-123456");

    fireEvent.click(screen.getByRole("button", { name: /mark ship release notes done/i }));
    expect(spies.advanceTask).toHaveBeenCalledWith("task-progress-123456");

    expect(screen.getByText("Archive old plan")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /archive old plan/i }),
    ).not.toBeInTheDocument();
  });

  it("is honest when the tasks module is not backed by the node", () => {
    renderTasks({
      tasks: [],
      status: {
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: 8,
        modules: [{ id: "chat", root: "bb".repeat(32) }],
      },
    });

    expect(screen.getByText(/tasks module is not available/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/task title/i)).toBeDisabled();
    expect(screen.getByRole("button", { name: /add task/i })).toBeDisabled();
  });
});
