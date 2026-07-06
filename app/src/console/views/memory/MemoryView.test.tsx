import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { LsEntry } from "../../../domain/memory-client";
import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import { createInitialState, type ConsoleState } from "../../store/state";
import { MemoryView } from "./MemoryView";

const entries: LsEntry[] = [
  { dir: { path: "/projects" } },
  {
    file: {
      path: "/notes.md",
      latest_generation: 3,
      generations: 3,
      latest_meta: {},
      latest_author: "aa".repeat(32),
      latest_published_at_height: 10,
      body_len: 42,
    },
  },
];

const renderMemory = (patch: Partial<ConsoleState> = {}) => {
  const initialState = {
    ...createInitialState(),
    connected: true,
    status: {
      version: "0.1.0",
      appHash: "aa".repeat(32),
      height: 8,
      modules: [{ id: "memory", root: "bb".repeat(32) }],
    },
    memoryPath: "/",
    memoryEntries: entries,
    ...patch,
  };
  const spies: Record<string, (...args: unknown[]) => void> = {};
  const noop = vi.fn() as (...args: unknown[]) => void;

  function Harness() {
    const [state] = useState(initialState);
    const actions = new Proxy(
      {},
      {
        get: (_target, key: string) => {
          spies[key] ??= vi.fn() as (...args: unknown[]) => void;
          return spies[key] ?? noop;
        },
      },
    ) as ConsoleActions;
    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <MemoryView />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { spies };
};

describe("MemoryView", () => {
  it("opens a file and browses a directory from the entry list", () => {
    const { spies } = renderMemory();

    fireEvent.click(screen.getByRole("button", { name: "Open /notes.md" }));
    expect(spies.openMemoryFile).toHaveBeenCalledWith({ path: "/notes.md" });

    fireEvent.click(screen.getByRole("button", { name: "Browse /projects" }));
    expect(spies.browseMemory).toHaveBeenCalledWith("/projects");
  });

  it("is honest when the memory module is not backed by the node", () => {
    renderMemory({
      memoryEntries: [],
      status: {
        version: "0.1.0",
        appHash: "aa".repeat(32),
        height: 8,
        modules: [{ id: "chat", root: "bb".repeat(32) }],
      },
    });

    expect(screen.getByText(/memory module is not available/i)).toBeInTheDocument();
  });
});
